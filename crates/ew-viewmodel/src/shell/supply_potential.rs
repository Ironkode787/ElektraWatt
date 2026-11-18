// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The supply marks' Potential panel (`ip.supply_potential`, H9): when supply marks are placed (points marked with
//! Supply comes in here…, or external points the Interruption tool places), one or a group in one go, a small panel
//! opens at them with a row per mark: its name, the kind of conductor (L1, L2, L3, L, N, PE, L+, M, …) and the voltage
//! with AC or DC (`230 V AC`), pre-filled from the name where the name tells it ([`from_name`]). Enter takes every
//! row: a mark without a definition gets one (`potential.define`, named as the mark), a mark with one has it changed
//! (`fields.set_many`), all in one undo step; Escape places the marks without a potential, as before. A mark whose
//! potential has a type already (it continues a potential drawn elsewhere) takes that potential and is not asked.
//! The same panel opens from a mark's right-click (Potential…) for a later change, with every selected mark.

use ew_api::AnyCommand;
use ew_api::output::{PageTables, Uuid};
use serde_json::{Map, Value, json};

use super::{Mode, Outcome, ShellModel, failed, string_param};
use crate::actions::ActionItem;
use crate::key;
use crate::strings;

/// The action.
const SUPPLY_POTENTIAL: &str = "ip.supply_potential";

/// One row of the panel: a supply mark and what it carries, as typed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SupplyRow {
    /// The mark (its interruption point's function).
    pub point: Uuid,
    /// Its representation on the page shown (where the panel stands).
    pub rep: Uuid,
    /// Its name.
    pub name: String,
    /// The kind of conductor as the type's stored word (`l1`, `n`, `pe`, `l_plus`, `m`); empty: none.
    pub kind: String,
    /// The voltage as typed (`230 V AC`, `24 V DC`); empty: none.
    pub voltage: String,
    /// The definition at the mark, changed rather than made anew.
    def: Option<Uuid>,
}

/// The panel: its rows and what the last Enter was refused with.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SupplyPanel {
    /// The rows, one per mark, in the order of the marks on the page.
    pub rows: Vec<SupplyRow>,
    /// Why the last Enter took nothing, in words.
    pub error: Option<String>,
}

/// The kind of conductor and the voltage a mark's name tells (H9): the part before a dash (`L1-EXT` is `L1`), read as
/// `L1`, `L2`, `L3` (a phase, at `phase`, the project supply's voltage), `L` (a phase), `N`, `PE`, `PEN`, `L+`, `L-`,
/// `M` or `0V` (the 0 V), or `<n>V` (`24V`: a plus of that voltage, DC; `<n>VAC`: a phase of it); anything else tells
/// nothing (empty texts).
pub fn from_name(name: &str, phase: Option<&str>) -> (String, String) {
    let upper = name.trim().to_uppercase().replace('\u{2212}', "-");
    // The part before a dash that has something after it ("L-" is the minus itself).
    let head = match upper.find('-') {
        Some(i) if i > 0 && i + 1 < upper.len() => &upper[..i],
        _ => upper.as_str(),
    };
    let phase = || phase.unwrap_or_default().to_owned();
    let (kind, voltage) = match head {
        "L1" => ("l1", phase()),
        "L2" => ("l2", phase()),
        "L3" => ("l3", phase()),
        "L" => ("l", phase()),
        "N" => ("n", String::new()),
        "PE" => ("pe", String::new()),
        "PEN" => ("pen", String::new()),
        "L+" => ("l_plus", String::new()),
        "L-" => ("l_minus", String::new()),
        "M" | "0V" => ("m", String::new()),
        other => {
            let digits: String = other
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            match &other[digits.len()..] {
                _ if digits.is_empty() || digits.parse::<f64>().is_err() => ("", String::new()),
                "V" | "VDC" => ("l_plus", format!("{digits} V DC")),
                "VAC" => ("l", format!("{digits} V AC")),
                _ => ("", String::new()),
            }
        }
    };
    (kind.to_owned(), voltage)
}

/// A row's voltage after its kind of conductor became `kind` (rc.5): a conductor without a voltage of its own (N, PE,
/// PEN, FE) has none, so a voltage left from the kind before (a phase's `230 V AC`) goes; a phase without a voltage
/// takes the project supply's (`phase`); any other keeps `voltage` as typed.
fn voltage_for_kind(kind: &str, voltage: &str, phase: Option<&str>) -> String {
    match kind {
        "n" | "pe" | "pen" | "fe" => String::new(),
        "l1" | "l2" | "l3" | "l" if voltage.trim().is_empty() => phase.unwrap_or_default().to_owned(),
        _ => voltage.to_owned(),
    }
}

impl ShellModel {
    /// The Potential panel, while it is open.
    pub fn supply_panel(&self) -> Option<&SupplyPanel> {
        self.supply_panel.as_ref()
    }

    /// The kinds of conductor the panel offers: the stored words, an empty one (none) first, and each as the window
    /// words it.
    pub fn supply_kinds(&self) -> Vec<(String, String)> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut out = vec![(String::new(), String::new())];
        out.extend(
            project
                .session
                .field_choices("potential_def", "ptype", "")
                .into_iter()
                .filter_map(|c| c.value.as_str().map(str::to_owned))
                .map(|w| {
                    let text = strings::choice_text(&w);
                    (w, text)
                }),
        );
        out
    }

    /// The panel's row for the mark drawn by `rep` (an interruption point's representation on `tables`' page), as it
    /// stands; `None` for any other representation, and with `skip_typed` for a mark whose potential has a type already.
    fn supply_row(&self, tables: &PageTables, rep: Uuid, skip_typed: bool) -> Option<SupplyRow> {
        let project = self.project.as_ref()?;
        let r = tables.reps.iter().find(|r| r.id == rep)?;
        r.ip.as_ref()?;
        let point = *r.functions.first()?;
        let mark = project.session.supply_point(point)?;
        if skip_typed && mark.typed {
            return None;
        }
        let (kind, voltage) = if mark.def.is_some() {
            (mark.ptype, mark.voltage)
        } else {
            from_name(&mark.name, project.session.supply_phase_voltage().as_deref())
        };
        Some(SupplyRow {
            point,
            rep,
            name: mark.name,
            kind,
            voltage,
            def: mark.def,
        })
    }

    /// After a commit (`after_commit`): the supply marks the commit placed on the page shown (a representation of an
    /// external point it made or made external) whose potential has no type yet open the panel, in the window only.
    pub(super) fn after_supply_marks(&mut self, before: &PageTables, now: &PageTables) {
        if self.mode != Mode::Gui || self.dialog.is_some() {
            return;
        }
        let was_mark = |id: Uuid| {
            before
                .reps
                .iter()
                .any(|r| r.id == id && r.ip.as_ref().is_some_and(|ip| ip.external))
        };
        let placed: Vec<Uuid> = now
            .reps
            .iter()
            .filter(|r| r.ip.as_ref().is_some_and(|ip| ip.external) && !was_mark(r.id))
            .map(|r| r.id)
            .collect();
        let rows: Vec<SupplyRow> = placed
            .into_iter()
            .filter_map(|rep| self.supply_row(now, rep, true))
            .collect();
        if !rows.is_empty() {
            self.supply_panel = Some(SupplyPanel { rows, error: None });
        }
    }

    /// Potential…, for selected supply marks (the right-click menu's entry after Remove the supply mark); `None`
    /// otherwise.
    pub fn supply_potential_item(&self) -> Option<ActionItem> {
        let (_, marked) = self.selected_supply_points()?;
        self.menu_item(SUPPLY_POTENTIAL).filter(|_| marked)
    }

    /// `ip.supply_potential {row?, kind?, voltage?, apply?}` (see the module documentation): without parameters it
    /// opens the panel for the selected supply marks; `row` with `kind` or `voltage` changes that row's cell; `apply`
    /// takes every row. Escape (`ui.dismiss`) closes the panel.
    pub(crate) fn act_supply_potential(&mut self, p: &Map<String, Value>) -> Outcome {
        if p.get("apply").and_then(Value::as_bool) == Some(true) {
            return self.apply_supply_panel();
        }
        if let Some(row) = p.get("row").and_then(Value::as_u64) {
            let (kind, voltage) = match (string_param(p, "kind"), string_param(p, "voltage")) {
                (Err(e), _) | (_, Err(e)) => return e,
                (Ok(k), Ok(v)) => (k.map(str::to_owned), v.map(str::to_owned)),
            };
            let phase = self.project_session().and_then(ew_api::Session::supply_phase_voltage);
            let Some(r) = self
                .supply_panel
                .as_mut()
                .and_then(|panel| panel.rows.get_mut(usize::try_from(row).unwrap_or(usize::MAX)))
            else {
                return Outcome::refused(key!("reason-supply-potential-row"));
            };
            if let Some(kind) = kind {
                r.voltage = voltage_for_kind(&kind, &r.voltage, phase.as_deref());
                r.kind = kind;
            }
            if let Some(voltage) = voltage {
                r.voltage = voltage;
            }
            return Outcome::Done;
        }
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-supply-potential-select"));
        };
        let tables = std::sync::Arc::clone(&c.page().tables);
        let reps: Vec<Uuid> = self.tools.selection.items.iter().map(|e| e.0).collect();
        let rows: Option<Vec<SupplyRow>> = reps
            .into_iter()
            .map(|rep| self.supply_row(&tables, rep, false))
            .collect();
        match rows.filter(|r| !r.is_empty()) {
            Some(rows) => {
                self.supply_panel = Some(SupplyPanel { rows, error: None });
                Outcome::Done
            }
            None => Outcome::refused(key!("reason-supply-potential-select")),
        }
    }

    /// Enter: every row with a kind or a voltage defines its mark's potential (named as the mark) or changes the
    /// definition at it, in one undo step, all rows or none (a later row refused takes the earlier ones back); a text a
    /// cell does not take keeps the panel open with the refusal in words. A mark deleted meanwhile is left out; with
    /// every mark gone the panel closes and says so.
    fn apply_supply_panel(&mut self) -> Outcome {
        let Some(mut panel) = self.supply_panel.clone() else {
            return Outcome::refused(key!("reason-supply-potential-select"));
        };
        let gone = |id: Uuid| self.project_session().is_none_or(|s| !s.record_exists(id));
        panel.rows.retain(|r| !gone(r.point) && r.def.is_none_or(|d| !gone(d)));
        if panel.rows.is_empty() {
            self.supply_panel = None;
            return Outcome::refused(key!("reason-supply-potential-gone"));
        }
        let mut commands: Vec<AnyCommand> = Vec::new();
        let mut edits: Vec<Value> = Vec::new();
        for row in &panel.rows {
            let (kind, voltage) = (row.kind.trim(), row.voltage.trim());
            if kind.is_empty() && voltage.is_empty() && row.def.is_none() {
                continue;
            }
            let mut ptype = Value::Null;
            let (mut volts, mut ac_dc) = (Value::Null, Value::Null);
            if !kind.is_empty() {
                match self.potential_field("type", "ptype", kind) {
                    Ok((v, _)) => ptype = v,
                    Err(out) => return self.refuse_supply_panel(row, out),
                }
            }
            if !voltage.is_empty() {
                match self.potential_field("voltage", "voltage", voltage) {
                    Ok((v, current)) => {
                        volts = v;
                        ac_dc = current.unwrap_or(Value::Null);
                    }
                    Err(out) => return self.refuse_supply_panel(row, out),
                }
            }
            match row.def {
                None => {
                    let mut params = json!({
                        "at": {"cp": {"function": row.point.to_string(), "idx": 0}},
                        "name": row.name,
                    });
                    for (k, v) in [("ptype", &ptype), ("voltage", &volts)] {
                        if !v.is_null() {
                            params[k] = v.clone();
                        }
                    }
                    if !ac_dc.is_null() {
                        params["env"] = json!({ "ac_dc": ac_dc });
                    }
                    commands.push(AnyCommand {
                        name: "potential.define".into(),
                        params,
                    });
                }
                Some(def) => {
                    for (path, value) in [("ptype", ptype), ("voltage", volts), ("env.ac_dc", ac_dc)] {
                        edits.push(json!({
                            "kind": "potential_def", "targets": [def.to_string()], "path": path, "value": value,
                        }));
                    }
                }
            }
        }
        if !edits.is_empty() {
            commands.push(AnyCommand {
                name: "fields.set_many".into(),
                params: json!({ "edits": edits }),
            });
        }
        self.supply_panel = None;
        let mut committed = false;
        for cmd in &commands {
            let rev = self.revision();
            let out = self.commit_from_host(cmd);
            if out != Outcome::Done {
                // All rows or none: the rows taken before it go back, and the panel stays with the refusal.
                if committed {
                    let _ = self.step_history(false);
                }
                let why = match &out {
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Refused { reason_key } => strings::text(*reason_key).to_owned(),
                    _ => String::new(),
                };
                panel.error = Some(why);
                self.supply_panel = Some(panel);
                return out;
            }
            // One undo step for the whole panel.
            if self.revision() != rev {
                if committed && let Some(p) = &mut self.project {
                    p.session.fold_last_step();
                }
                committed = true;
            }
        }
        Outcome::Done
    }

    /// The panel stays open with the refusal of `row`'s cell, which names the mark.
    fn refuse_supply_panel(&mut self, row: &SupplyRow, out: Outcome) -> Outcome {
        let why = match &out {
            Outcome::Failed { message } => message.clone(),
            Outcome::Refused { reason_key } => strings::text(*reason_key).to_owned(),
            _ => String::new(),
        };
        let message = strings::format(key!("supply-potential-refused"), &[&row.name, &why]);
        if let Some(panel) = &mut self.supply_panel {
            panel.error = Some(message.clone());
        }
        failed(key!("supply-potential-refused"), &[&row.name, &why])
    }
}

#[cfg(test)]
mod tests {
    use super::{from_name, voltage_for_kind};

    /// A row made N or PE drops the phase's voltage it had; one made a phase takes the supply's where it had none.
    #[test]
    fn a_kind_of_conductor_without_a_voltage_drops_the_one_before() {
        let phase = Some("230 V AC");
        assert_eq!(voltage_for_kind("n", "230 V AC", phase), "");
        assert_eq!(voltage_for_kind("pe", "230 V AC", phase), "");
        assert_eq!(voltage_for_kind("l1", "", phase), "230 V AC");
        assert_eq!(voltage_for_kind("l1", "400 V AC", phase), "400 V AC");
        assert_eq!(voltage_for_kind("l_plus", "24 V DC", phase), "24 V DC");
    }

    #[test]
    fn a_mark_s_name_tells_its_conductor() {
        let phase = Some("400 V AC");
        for (name, kind, voltage) in [
            ("L1", "l1", "400 V AC"),
            ("L2-EXT", "l2", "400 V AC"),
            ("L-EXT", "l", "400 V AC"),
            ("N", "n", ""),
            ("PE-EXT", "pe", ""),
            ("24V", "l_plus", "24 V DC"),
            ("0V", "m", ""),
            ("M", "m", ""),
            ("L-", "l_minus", ""),
            ("230VAC", "l", "230 V AC"),
            ("Feed", "", ""),
        ] {
            assert_eq!(
                from_name(name, phase),
                (kind.to_owned(), voltage.to_owned()),
                "{name}"
            );
        }
        // Without supply settings a phase has no voltage.
        assert_eq!(from_name("L3", None), ("l3".to_owned(), String::new()));
    }
}
