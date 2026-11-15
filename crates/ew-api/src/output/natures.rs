// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a device is (rc.2, the owner's F9 and F12): its natures, derived from what the model knows of it — the
//! disciplines of its functions' connection points, its bus ports and its aspects. One notion feeds the Properties
//! header ("Compressed-air supply point · pneumatic"), the rows Properties shows and the entries of its menus
//! ("Define potential…" on an electrical point only, "Medium ▸" on a fluid source); [`Session::device_natures`]
//! gives it to a script and the API.

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{FunctionCps, Q, is_fluid};
use ew_schema::{Device, Discipline, StationRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Session;

/// The natures of one device: what it is, as Properties and the menus read it.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Natures {
    /// The disciplines of its functions' connection points, each once, in the order they are met (a valve with its
    /// solenoid: pneumatic and electrical).
    pub disciplines: Vec<Discipline>,
    /// Whether one of its functions is a bus port (a station's, a switch's, a bus device's).
    pub bus: bool,
    /// Its role when it is a PLC station (a CPU, a valve terminal, a head station).
    pub station: Option<StationRole>,
    /// Whether it is a PLC module (a card in a station's rack).
    pub module: bool,
    /// Whether it is a terminal.
    pub terminal: bool,
    /// Whether it is a terminal strip.
    pub strip: bool,
    /// Whether it is a cable.
    pub cable: bool,
}

impl Natures {
    /// Whether one of its connection points is electrical.
    pub fn electrical(&self) -> bool {
        self.disciplines.contains(&Discipline::Electrical)
    }

    /// Whether one of its connection points is a fluid port (pneumatic, vacuum or hydraulic).
    pub fn fluid(&self) -> bool {
        self.disciplines.iter().any(is_fluid)
    }

    /// Whether it takes part in a bus: a bus port, a PLC station or module.
    pub fn bus_device(&self) -> bool {
        self.bus || self.station.is_some() || self.module
    }

    /// Whether it is none of the other natures: a device with no connection point and no aspect (a mounting rail, a
    /// sign).
    pub fn mechanical_only(&self) -> bool {
        self.disciplines.is_empty() && !self.bus_device() && !self.terminal && !self.strip && !self.cable
    }
}

/// The natures of `device`; the default (mechanical only) for an ID that names no device.
pub(crate) fn device_natures(q: &Q, device: Id<Device>) -> Natures {
    let Some(d) = q.devices().get(device) else {
        return Natures::default();
    };
    let functions = q.functions();
    let mut out = Natures {
        station: d.aspects.plc_station.as_ref().map(|s| s.role.clone()),
        module: d.aspects.plc_module.is_some(),
        terminal: d.aspects.terminal.is_some(),
        strip: d.aspects.strip.is_some(),
        cable: d.aspects.cable.is_some(),
        ..Natures::default()
    };
    for f in q.idx().functions_of_device(device) {
        if functions.get(f).is_some_and(|x| x.bus_port.is_some()) {
            out.bus = true;
        }
        for cp in q.get::<FunctionCps>(f).iter() {
            if cp.discipline == Discipline::Bus {
                out.bus = true;
            } else if !out.disciplines.contains(&cp.discipline) {
                out.disciplines.push(cp.discipline.clone());
            }
        }
    }
    out
}

impl Session {
    /// What the device `device` is ([`Natures`]); `None` for an ID that names no device.
    pub fn device_natures(&self, device: Uuid) -> Option<Natures> {
        let q = self.query();
        let id = Id::from_uuid(device);
        q.devices().get(id)?;
        Some(device_natures(&q, id))
    }
}
