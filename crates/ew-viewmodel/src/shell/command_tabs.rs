// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC and Fluid tabs' own entries (doc 16a §5.3): each runs an action the window already has with the
//! parameters of its discipline, so that nothing of the two disciplines is reached through a menu or a row menu only.
//!
//! - **The workspace switch** (`plc.workspace`, `fluid.workspace`): loads the discipline's workspace, or Schematic
//!   when it is the active one.
//! - **The guide** (`plc.guide`, `fluid.guide`): the discipline's how-to guide (`help.documentation {topic}`).
//! - **A panel** (`plc.grid`, `bus.panel`, `fluid.tubes`, `fluid.media`): shows the
//!   navigator, or hides it when it is shown (`navigator.show {kind, on}`).
//! - **A page** (`bus.page_new`, `fluid.page_new`): New page… with the page type chosen.
//! - **A picker** (`fluid.symbol`, `fluid.air_supply`, `fluid.valve_terminal`): the symbol or device browser opened
//!   at the discipline's things.
//! - **Place port** (`bus.place_port {device, port}`): a port the Bus panel lists as not drawn yet, as its row does.

use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::navigators::NavKind;
use crate::tools::PickerMode;

/// A JSON object value as a parameter map.
fn params(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

impl ShellModel {
    /// The workspace switch of a discipline: loads `name`, or Schematic when `name` is the active workspace.
    pub(crate) fn act_discipline_workspace(&mut self, name: &str) -> Outcome {
        let to = if self.workspace() == name {
            crate::settings::DEFAULT_WORKSPACE
        } else {
            name
        };
        self.act_workspace_load(&params(json!({ "name": to })))
    }

    /// The guide of a discipline: the help topic `topic`.
    pub(crate) fn act_discipline_guide(&mut self, topic: &str) -> Outcome {
        self.act_documentation(&params(json!({ "topic": topic })))
    }

    /// Shows the navigator `kind`, or hides it when it is shown.
    pub(crate) fn act_discipline_panel(&mut self, kind: NavKind) -> Outcome {
        let on = !self.navigator_open(kind);
        self.act_navigator_show(&params(json!({ "kind": kind.word(), "on": on })))
    }

    /// New page… with the page type `kind` chosen.
    pub(crate) fn act_discipline_page(&mut self, kind: &str) -> Outcome {
        self.act_page_new(&params(json!({ "type": kind })))
    }

    /// The browser of `mode` opened at the function category `category` (`AirPreparation`).
    pub(crate) fn act_discipline_picker(&mut self, mode: PickerMode, category: &str) -> Outcome {
        let out = self.act_open_picker(mode);
        if out != Outcome::Done {
            return out;
        }
        self.act_picker_category(&params(json!({ "to": category })))
    }

    /// The PLC stations with their tags, in natural order of the tag: the PLC tab's Rack and slots… lists them, as each
    /// station's row of the PLC panel offers it.
    pub(crate) fn plc_stations(&self) -> Vec<(ew_api::output::Uuid, String)> {
        let Some(session) = self.project_session() else {
            return Vec::new();
        };
        let mut out: Vec<(ew_api::output::Uuid, String)> = session
            .query()
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.plc_station.is_some())
            .map(|(id, _)| {
                let id = id.as_uuid();
                (id, session.device_tag(id).unwrap_or_default())
            })
            .collect();
        out.sort_by(|a, b| super::natural_cmp(&a.1, &b.1));
        out
    }

    /// `bus.place_port {device, port}`: Place port of a port not drawn yet, on a bus-topology page.
    pub(crate) fn act_bus_place_port(&mut self, p: &Map<String, Value>) -> Outcome {
        let get = |name: &str| match string_param(p, name) {
            Ok(Some(v)) => Ok(v.to_owned()),
            Ok(None) => Err(failed(key!("error-missing-param"), &[name])),
            Err(e) => Err(e),
        };
        let device = or_return!(get("device"));
        let port = or_return!(get("port"));
        self.place_undrawn_port(&device, &port)
    }
}
