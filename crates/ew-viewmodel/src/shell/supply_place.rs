// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Supply (`supply.place`, rc.5): the Draw tab's button for what comes in from the mains. Its form asks what comes in
//! (single phase L, N, PE; three phase L1, L2, L3, N, PE; three phase without N; 24 V DC and 0 V; own names) and where
//! it comes from (the place and the device there, as Supply comes in here… proposes them). Apply floats the marks as one
//! group on the pointer ([`SupplyTool`]); the click places them, marks them as where the supply comes in
//! (`ip.supply_in`) in one undo step, and the marks' Potential panel opens, pre-filled from their names. Supply comes
//! in here… on points drawn with Interruption stays.

use ew_api::AnyCommand;
use ew_api::output::Uuid;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::tools::supply_place::{SupplyPlan, SupplyTool};

/// The action.
const SUPPLY_PLACE: &str = "supply.place";

/// The marks of a system's word (`supply_1p`, `supply_3p`, `supply_3p_no_n`, `supply_dc`); `supply_own` takes the
/// names typed, separated by
/// commas or spaces. `None` for any other word or no name.
pub fn supply_names(system: &str, own: Option<&str>) -> Option<Vec<String>> {
    let names: Vec<String> = match system {
        "supply_1p" => ["L", "N", "PE"].map(str::to_owned).to_vec(),
        "supply_3p" => ["L1", "L2", "L3", "N", "PE"].map(str::to_owned).to_vec(),
        "supply_3p_no_n" => ["L1", "L2", "L3", "PE"].map(str::to_owned).to_vec(),
        "supply_dc" => ["24V", "0V"].map(str::to_owned).to_vec(),
        "supply_own" => own?
            .split([',', ' '])
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => return None,
    };
    (!names.is_empty()).then_some(names)
}

/// The form's names field comes with Own, right after the system, and goes with another system (rc.5: "Own names
/// (for Own)" stood in every form and two runs did not understand it).
pub(super) fn own_names_field(form: &mut FormDialog) {
    if form.action != SUPPLY_PLACE {
        return;
    }
    let own = form
        .fields
        .iter()
        .any(|f| f.name == "system" && f.value == "supply_own");
    match (own, form.fields.iter().position(|f| f.name == "names")) {
        (true, None) => {
            let after = form
                .fields
                .iter()
                .position(|f| f.name == "system")
                .map_or(0, |i| i + 1);
            form.fields.insert(
                after,
                FormField {
                    name: "names",
                    label_key: key!("supply-place-names"),
                    choices: &super::choices::NONE,
                    options: Vec::new(),
                    value: String::new(),
                },
            );
        }
        (false, Some(i)) => {
            form.fields.remove(i);
        }
        _ => {}
    }
}

impl ShellModel {
    /// `supply.place {system?, names?, location?, device?}` (see the module documentation). In the window without
    /// `system` its form opens; with it the Supply tool floats the marks.
    pub(crate) fn act_supply_place(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = |name: &str| match string_param(p, name) {
            Ok(v) => Ok(v.map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned)),
            Err(e) => Err(e),
        };
        let (system, names, location, device) =
            match (text("system"), text("names"), text("location"), text("device")) {
                (Ok(s), Ok(n), Ok(l), Ok(d)) => (s, n, l, d),
                (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => return e,
            };
        let (place, feeder) = self.supply_in_proposal();
        let Some(system) = system else {
            if self.mode == Mode::Gui && !self.in_form_of(SUPPLY_PLACE) {
                let field = |name, label_key, choices, value: String| FormField {
                    name,
                    label_key,
                    choices,
                    options: Vec::new(),
                    value,
                };
                return self.open_dialog(Dialog::Form(FormDialog {
                    action: SUPPLY_PLACE,
                    title_key: key!("supply-place-title"),
                    ok_key: key!("supply-place-ok"),
                    fields: vec![
                        field(
                            "system",
                            key!("supply-place-system"),
                            &super::choices::SUPPLY_SYSTEMS,
                            "supply_1p".to_owned(),
                        ),
                        field(
                            "location",
                            key!("supply-in-location"),
                            &super::choices::NONE,
                            place,
                        ),
                        field("device", key!("supply-in-device"), &super::choices::NONE, feeder),
                    ],
                    error: None,
                    table: None,
                }));
            }
            return failed(key!("error-missing-param"), &["system"]);
        };
        let Some(names) = supply_names(&system, names.as_deref()) else {
            let message = strings_text(key!("supply-place-no-names"));
            if self.in_form_of(SUPPLY_PLACE) {
                return self.form_failed(SUPPLY_PLACE, message);
            }
            return Outcome::Failed { message };
        };
        let (shape, target, copies) = match self.ip_forms() {
            Ok(f) => f,
            Err(out) => return out,
        };
        self.tools.supply = Some(SupplyPlan {
            location: location.unwrap_or(place),
            device: device.unwrap_or(feeder),
            copies,
        });
        self.dialog = None;
        self.activate_tool(Box::new(SupplyTool::new(names, target.unwrap_or(shape))));
        Outcome::Done
    }

    /// The Supply tool's click at `at` (page nm): one interruption point per mark, a target named as the mark, one
    /// under the other, marked as where the supply comes in from the plan's place and device; one undo step. The
    /// marks' Potential panel opens (`after_supply_marks`), and Select takes over.
    pub(super) fn supply_place_from_tool(&mut self, at: (i64, i64)) -> Outcome {
        let Some(plan) = self.tools.supply.clone() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let placed = self
            .with_tool(|t, cx| {
                let tool = t.as_any_mut()?.downcast_mut::<SupplyTool>()?;
                Some((
                    tool.names().to_vec(),
                    tool.shape().copy.to_string(),
                    tool.positions(at, cx),
                    cx.page.tables.page.to_string(),
                ))
            })
            .flatten();
        let Some((names, symbol, positions, page)) = placed else {
            return Outcome::Done;
        };
        let mut points: Vec<Uuid> = Vec::new();
        let mut first = true;
        for (name, pos) in names.iter().zip(&positions) {
            let mut params = json!({
                "page": page,
                "symbol": symbol,
                "pos": [pos.0, pos.1],
                "orient": "r0",
                "name": name,
                "direction": "target",
            });
            if first && !plan.copies.is_empty() {
                params["copies"] = Value::Array(plan.copies.clone());
            }
            let rev = self.revision();
            let out = self.commit_from_host(&AnyCommand {
                name: "ip.place".into(),
                params,
            });
            if out != Outcome::Done {
                return out;
            }
            if !first && self.revision() != rev {
                self.fold_step();
            }
            first = false;
            self.sync_views();
            let point = self.canvas().and_then(|c| {
                c.page()
                    .tables
                    .reps
                    .iter()
                    .find(|r| r.pos == *pos && r.ip.as_ref().is_some_and(|ip| ip.name == *name))
                    .and_then(|r| r.functions.first().copied())
            });
            points.extend(point);
        }
        let rev = self.revision();
        let out = self.commit_from_host(&AnyCommand {
            name: "ip.supply_in".into(),
            params: json!({
                "points": points.iter().map(Uuid::to_string).collect::<Vec<_>>(),
                "location": plan.location,
                "device": plan.device,
            }),
        });
        if out == Outcome::Done && self.revision() != rev {
            self.fold_step();
        }
        self.tools.supply = None;
        let _ = self.act_tool("select");
        out
    }

    /// Folds the newest undo step into the one below it.
    fn fold_step(&mut self) {
        if let Some(p) = &mut self.project {
            p.session.fold_last_step();
        }
    }
}

fn strings_text(k: crate::strings::Key<'static>) -> String {
    crate::strings::text(k).to_owned()
}
