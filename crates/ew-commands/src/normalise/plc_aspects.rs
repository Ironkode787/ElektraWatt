// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #0, PLC aspects from the part (N0; WP-S1.21 D2 to D4, doc 06 §3.2, doc 10 §3.3).
//!
//! **Trigger.** A device of this transaction that carries neither `plc_module` nor `plc_station` and whose part
//! with a PLC section is new: the first part use whose part has a PLC section (the rule of
//! `ew_derive::module_part` for a device without `plc_module`) names another part copy than it did in the base
//! snapshot, or the device is new. So placing a part, creating a device with parts, `part.assign`, `fields.set` on
//! `parts`, a paste or a macro all reach it through the registry; a device whose aspect a person removed keeps no
//! aspect until its PLC part changes, and an edit of an unrelated member never adds one.
//!
//! **Effect** (D2). From the section's role:
//! - `module`: `plc_module {slot: positional, start: auto}` without `part_use` (absent names the first part use
//!   with a PLC section, which is the one N0 read). Then D4, for a device without a station ancestor: with exactly
//!   one station in the project, the device becomes that station's last child (`parent` = the station, `position`
//!   = its highest child position + 1, `assembly.attach`'s members) with the notice "attached to {tag} as module
//!   {n}"; with none, the notice "no PLC station: place a CPU first"; with several, "attach the module to a
//!   station". A station that lies below the module (a part change on an assembly) is never chosen: that parent
//!   would close a cycle (EW-INV-009).
//! - `station`: `plc_station {config_project, role: cpu}` and, when the section declares I/O ranges,
//!   `plc_module {integrated: true}` on the same device. The configuration project (D3): with none in the project,
//!   one is created in the same transaction, named `PLC<n>` (the lowest free n), pinned to the shipped TIA
//!   international profile, `cpu` the station, ID `v5(station, "plc_config_project")`; with one, the station joins
//!   it; with several, the first by (name, ID). Joining gives a notice that names the configuration project.
//! - a role of a newer format: nothing.
//!
//! N0 runs before N1 (`channel_instantiation`), so the new module's channels exist in the same pass and N3 binds
//! their pins. It never removes or rewrites an aspect (removing one stays `fields.set`). Stations are handled before
//! modules, so a station made in the same transaction counts for D4; modules in the order of their main
//! representations' anchor cells (`numbering::place_key`, ties by ID), so a replay attaches them at the same
//! positions.
//!
//! Idempotent: after a run every triggered device carries an aspect, so a second run finds nothing to do.

use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::Ident;
use ew_schema::{
    Device, Extra, Kind, LibCopy, Part, PartUse, PlcConfigProject, PlcModule, PlcRole, PlcSection,
    PlcStation, StationRole,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #0.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlcAspects;

/// The first part use of `parts` whose part copy has a PLC section: its copy and the section.
fn plc_part(tx: &TxBuilder<'_>, parts: &[PartUse]) -> Option<(Id<Part>, PlcSection)> {
    parts.iter().find_map(|u| {
        let copy: Arc<LibCopy<Part>> = tx.get::<LibCopy<Part>>(u.part.copy)?;
        let section = copy.item.sections.plc.clone()?;
        Some((u.part.copy, section))
    })
}

/// The PLC section N0 applies to `dev`, when the device is triggered (see the module documentation).
fn triggered(tx: &TxBuilder<'_>, dev: &Device) -> Option<PlcSection> {
    if dev.aspects.plc_module.is_some() || dev.aspects.plc_station.is_some() {
        return None;
    }
    let (copy, section) = plc_part(tx, &dev.parts)?;
    let before = tx
        .base_row(&(Kind::Device, dev.id.cast()))
        .and_then(|r| r.get::<Device>().cloned())
        .and_then(|b| plc_part(tx, &b.parts).map(|(c, _)| c));
    (before != Some(copy)).then_some(section)
}

/// A device's tag as a notice names it (`-A1`; `-A` before online numbering gives it a counter).
fn tag_text(dev: &Device) -> String {
    match dev.tag.counter {
        Some(c) => format!("-{}{c}", dev.tag.letter.as_str()),
        None => format!("-{}", dev.tag.letter.as_str()),
    }
}

/// Whether `device` or one of its ancestors carries `plc_station` (bounded, so a parent cycle terminates).
fn has_station_ancestor(tx: &TxBuilder<'_>, device: Id<Device>) -> bool {
    let mut at = Some(device);
    let mut steps = 0usize;
    while let Some(d) = at.and_then(|d| tx.get::<Device>(d)) {
        if d.aspects.plc_station.is_some() {
            return true;
        }
        steps += 1;
        if steps > 4096 {
            return false;
        }
        at = d.parent;
    }
    false
}

/// Whether `ancestor` is `device` or one of its ancestors (bounded).
fn is_at_or_above(tx: &TxBuilder<'_>, ancestor: Id<Device>, device: Id<Device>) -> bool {
    let mut at = Some(device);
    let mut steps = 0usize;
    while let Some(d) = at {
        if d == ancestor {
            return true;
        }
        steps += 1;
        if steps > 4096 {
            return true; // a cycle: never attach into it
        }
        at = tx.get::<Device>(d).and_then(|x| x.parent);
    }
    false
}

/// The configuration project a new station joins (D3), creating `PLC<n>` when the project has none.
fn config_project_for(tx: &mut TxBuilder<'_>, station: &Device) -> Result<Id<PlcConfigProject>, CmdError> {
    let mut all = tx.iter::<PlcConfigProject>();
    all.sort_by(|a, b| (a.name.as_str(), a.id).cmp(&(b.name.as_str(), b.id)));
    let tag = tag_text(station);
    if let Some(first) = all.first() {
        let more = if all.len() > 1 {
            format!(
                ", the first of {} by name; set `plc_station.config_project` to choose another",
                all.len()
            )
        } else {
            String::new()
        };
        tx.notice(format!(
            "station {tag} joins configuration project {}{more}",
            first.name
        ));
        return Ok(first.id);
    }
    let taken: std::collections::BTreeSet<&str> = all.iter().map(|c| c.name.as_str()).collect();
    let n = (1u32..)
        .find(|n| !taken.contains(format!("PLC{n}").as_str()))
        .unwrap_or(1);
    let name = Ident::from_input(&format!("PLC{n}"))
        .map_err(|e| CmdError::internal(format!("N0: the name PLC{n} is no identifier ({e:?})")))?;
    let profile = ew_derive::default_profile();
    let profile_ident = Ident::from_input(&profile)
        .map_err(|e| CmdError::internal(format!("N0: the profile {profile} is no identifier ({e:?})")))?;
    let id: Id<PlcConfigProject> = v5::plc_config_project_id(station.id);
    tx.insert(PlcConfigProject {
        id,
        v: 1,
        name: name.clone(),
        profile: profile_ident,
        cpu: Some(station.id),
        source_ref: None,
        extra: Extra::new(),
    })?;
    tx.notice(format!(
        "configuration project {name} created for station {tag} (profile {profile})"
    ));
    Ok(id)
}

/// D4: attaches a new module without a station ancestor to the project's only station, or says why not.
fn attach_module(tx: &mut TxBuilder<'_>, module: Id<Device>) -> Result<bool, CmdError> {
    if has_station_ancestor(tx, module) {
        return Ok(false);
    }
    let stations: Vec<Arc<Device>> = tx
        .iter::<Device>()
        .into_iter()
        .filter(|d| d.aspects.plc_station.is_some())
        .collect();
    let station = match stations.as_slice() {
        [] => {
            tx.notice("no PLC station: place a CPU first");
            return Ok(false);
        }
        [one] => Arc::clone(one),
        _ => {
            tx.notice("attach the module to a station");
            return Ok(false);
        }
    };
    if is_at_or_above(tx, module, station.id) {
        tx.notice("attach the module to a station");
        return Ok(false);
    }
    let position = tx
        .iter::<Device>()
        .into_iter()
        .filter(|d| d.parent == Some(station.id) && d.id != module)
        .filter_map(|d| d.position)
        .max()
        .map_or(1, |p| p.saturating_add(1));
    let parent = station.id;
    tx.update::<Device>(module, |d| {
        d.parent = Some(parent);
        d.position = Some(position);
    })?;
    tx.notice(format!("attached to {} as module {position}", tag_text(&station)));
    Ok(true)
}

impl Normaliser for PlcAspects {
    fn id(&self) -> &'static str {
        "plc_aspects"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut stations: Vec<(Arc<Device>, PlcSection)> = Vec::new();
        let mut modules: Vec<Arc<Device>> = Vec::new();
        for key in tx.changed_of(Kind::Device) {
            let Some(dev) = tx.get::<Device>(key.1.cast()) else {
                continue;
            };
            let Some(section) = triggered(tx, &dev) else {
                continue;
            };
            match section.role {
                PlcRole::Station => stations.push((dev, section)),
                PlcRole::Module => modules.push(dev),
                PlcRole::Unknown(_) => {}
            }
        }
        if stations.is_empty() && modules.is_empty() {
            return Ok(false);
        }
        stations.sort_by_key(|(d, _)| d.id);
        for (dev, section) in stations {
            let cfg = config_project_for(tx, &dev)?;
            let integrated = (!section.io.is_empty()).then(|| PlcModule {
                integrated: true,
                ..PlcModule::default()
            });
            tx.update::<Device>(dev.id, |d| {
                d.aspects.plc_station = Some(PlcStation {
                    config_project: cfg,
                    role: StationRole::Cpu,
                    order: None,
                    extra: Extra::new(),
                });
                if let Some(m) = integrated {
                    d.aspects.plc_module = Some(m);
                }
            })?;
        }
        let mut keyed: Vec<_> = modules
            .into_iter()
            .map(|d| (crate::numbering::place_key(tx, d.id), d.id))
            .collect();
        keyed.sort();
        for (_, module) in keyed {
            tx.update::<Device>(module, |d| d.aspects.plc_module = Some(PlcModule::default()))?;
            attach_module(tx, module)?;
        }
        Ok(true)
    }
}
