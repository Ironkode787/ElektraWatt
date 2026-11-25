// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #0, PLC aspects from the part (N0; doc 06 §3.2, doc 10 §3.3).
//!
//! **Trigger.** A device of this transaction that carries neither `plc_module` nor `plc_station` and whose part
//! with a PLC section is new: the first part use whose part has a PLC section (the rule of
//! `ew_derive::module_part` for a device without `plc_module`) names another part copy than it did in the base
//! snapshot, or the device is new. So placing a part, creating a device with parts, `part.assign`, `fields.set` on
//! `parts`, a paste or a macro all reach it through the registry; a device whose aspect a person removed keeps no
//! aspect until its PLC part changes, and an edit of an unrelated member never adds one. A part copy that the
//! transaction changes (`library.update`, which keeps the copy's ID across versions) triggers every device that
//! uses it when the copy had no PLC section in the base snapshot.
//!
//! **Effect**. From the section's role:
//! - `module`: `plc_module {slot: positional, start: auto}` without `part_use` (absent names the first part use
//!   with a PLC section, which is the one N0 read). Then D4, for a device without a station ancestor: with exactly
//!   one station in the project, the device becomes that station's last child (`parent` = the station, `position`
//!   = its highest child position + 1, `assembly.attach`'s members) with the notice "attached to {tag} as module
//!   {n}"; with none, the notice "no PLC station: place a CPU first"; with several, the station drawn beside it (a
//!   representation on the page of the module's first representation, the nearest by insertion point, ties by ID),
//!   else "attach the module to a station". A station that lies below the module (a part change on an assembly) is never chosen: that parent
//!   would close a cycle (EW-INV-009).
//! - `station`: `plc_station {config_project, role}` and, when the section declares I/O ranges,
//!   `plc_module {integrated: true}` on the same device. The role is `valve_terminal` when the section maps solenoids
//!   to valve positions (`positions`: the part carries valves); else `head_station` when the device's parts declare
//!   bus ports and none of them is a `master` or a `controller` port (a remote I/O head); else `cpu`. The
//!   configuration project: with none in the project, one is created in the same transaction, named `PLC<n>`
//!   (the lowest free n), pinned to the shipped TIA international profile, `cpu` the station when it is a CPU, ID
//!   `v5(station, "plc_config_project")`; with one, the station joins it (a CPU becomes its `cpu` when it has none);
//!   with several, the first by (name, ID). Joining gives a notice that names the configuration project. D4 never
//!   attaches a card to a valve terminal.
//! - a role of a newer format: nothing.
//!
//! **A station's arrival**. When this run gives a device `plc_station` and the project then holds exactly one
//! station, every module without a station ancestor (a card drawn before the CPU: N0 gave it `plc_module` with the
//! notice "no PLC station: place a CPU first") becomes that station's child by D4, in the order of the modules' main
//! representations' anchor cells, in the same transaction as the station. An integrated module is the station itself
//! and is left alone, as is a module the station lies below.
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
    PlcStation, PortRole, Representation, StationRole,
};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// Normaliser #0.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlcAspects;

/// The role of a new station: a valve terminal when its part's PLC section maps solenoids to valve positions (the
/// part says it carries valves, as the starter terminal EW-GEN-VTM-1 does); a head station when its parts declare bus
/// ports and none leads a bus (no `master` and no `controller` port); else a CPU.
pub(crate) fn station_role(tx: &TxBuilder<'_>, dev: &Device, section: &PlcSection) -> StationRole {
    if section.positions.is_some() {
        return StationRole::ValveTerminal;
    }
    let roles: Vec<PortRole> = dev
        .parts
        .iter()
        .filter_map(|u| tx.get::<LibCopy<Part>>(u.part.copy))
        .filter_map(|c| c.item.sections.bus.clone())
        .flat_map(|b| b.ports.into_iter().map(|p| p.role))
        .collect();
    let leads = roles
        .iter()
        .any(|r| matches!(r, PortRole::Master | PortRole::Controller));
    if roles.is_empty() || leads {
        StationRole::Cpu
    } else {
        StationRole::HeadStation
    }
}

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
    // the copy itself gained its PLC section in this transaction (`library.update`)
    let gained = tx
        .base_row(&(Kind::Part, copy.cast()))
        .and_then(|r| r.get::<LibCopy<Part>>().map(|c| c.item.sections.plc.is_none()))
        .unwrap_or(false);
    (before != Some(copy) || gained).then_some(section)
}

/// The devices that use a part copy this transaction changed, in ID order: `library.update` replaces a copy's content
/// under its ID, so N0 and N5 look at its devices as if their parts had changed.
pub(crate) fn devices_of_changed_parts(tx: &TxBuilder<'_>) -> Vec<Id<Device>> {
    let changed: std::collections::BTreeSet<Id<Part>> = tx
        .changed_of(Kind::Part)
        .into_iter()
        .map(|k| k.1.cast())
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    tx.iter::<Device>()
        .into_iter()
        .filter(|d| d.parts.iter().any(|u| changed.contains(&u.part.copy)))
        .map(|d| d.id)
        .collect()
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

/// Whether `d` is a station that takes cards attached by D4: any station but a valve terminal, whose children are its
/// valves.
fn takes_cards(d: &Device) -> bool {
    d.aspects
        .plc_station
        .as_ref()
        .is_some_and(|s| s.role != StationRole::ValveTerminal)
}

/// The configuration project a new station joins, creating `PLC<n>` when the project has none. A CPU is the
/// project's CPU when it creates it or joins one without; a valve terminal never is.
fn config_project_for(
    tx: &mut TxBuilder<'_>,
    station: &Device,
    role: &StationRole,
) -> Result<Id<PlcConfigProject>, CmdError> {
    let mut all = tx.iter::<PlcConfigProject>();
    all.sort_by(|a, b| (a.name.as_str(), a.id).cmp(&(b.name.as_str(), b.id)));
    let tag = ew_derive::text::letter_counter_tag(&station.tag);
    let cpu = (*role == StationRole::Cpu).then_some(station.id);
    if let Some(first) = all.first() {
        if first.cpu.is_none() && cpu.is_some() {
            tx.update::<PlcConfigProject>(first.id, |c| c.cpu = cpu)?;
        }
        tx.notice(if all.len() > 1 {
            text!(
                "notice-station-joins-first",
                tag,
                first.name.to_string(),
                all.len().to_string()
            )
        } else {
            text!("notice-station-joins", tag, first.name.to_string())
        });
        return Ok(first.id);
    }
    create_config_project(tx, station, cpu)
}

/// A new configuration project for `station`, `PLC<n>` (the lowest free n), pinned to the shipped TIA international
/// profile, `cpu` its CPU, ID `v5(station, "plc_config_project")`, with a notice that names it.
pub(crate) fn create_config_project(
    tx: &mut TxBuilder<'_>,
    station: &Device,
    cpu: Option<Id<Device>>,
) -> Result<Id<PlcConfigProject>, CmdError> {
    let all = tx.iter::<PlcConfigProject>();
    let tag = ew_derive::text::letter_counter_tag(&station.tag);
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
        cpu,
        symbols: None,
        ownership: None,
        exchange: None,
        source_ref: None,
        extra: Extra::new(),
    })?;
    tx.notice(text!("notice-config-created", name.to_string(), tag, profile));
    Ok(id)
}

/// The station of `stations` drawn beside `module` (a card placed beside its head station in a
/// project with a CPU too): the one with a representation on the page of the module's first representation nearest to
/// it (squared distance between insertion points, ties by station ID); `None` when no station is drawn on that page.
fn beside(tx: &TxBuilder<'_>, module: Id<Device>, stations: &[Arc<Device>]) -> Option<Arc<Device>> {
    let reps_of = |d: Id<Device>| -> Vec<Arc<Representation>> {
        let mut out: Vec<Arc<Representation>> = tx
            .functions_of_device(d)
            .into_iter()
            .flat_map(|f| tx.reps_of_function(f))
            .filter_map(|r| tx.get::<Representation>(r))
            .collect();
        out.sort_by_key(|r| r.id);
        out
    };
    let at = reps_of(module).into_iter().next()?;
    let dist = |r: &Representation| {
        let (dx, dy) = (
            i128::from(r.pos.x.0) - i128::from(at.pos.x.0),
            i128::from(r.pos.y.0) - i128::from(at.pos.y.0),
        );
        dx * dx + dy * dy
    };
    stations
        .iter()
        .filter_map(|s| {
            reps_of(s.id)
                .iter()
                .filter(|r| r.page == at.page)
                .map(|r| dist(r))
                .min()
                .map(|d| (d, s.id, Arc::clone(s)))
        })
        .min_by_key(|(d, id, _)| (*d, *id))
        .map(|(_, _, s)| s)
}

/// D4: attaches a new module without a station ancestor to the project's only station, else to the station drawn
/// beside it ([`beside`]), or says why not.
fn attach_module(tx: &mut TxBuilder<'_>, module: Id<Device>) -> Result<bool, CmdError> {
    if has_station_ancestor(tx, module) {
        return Ok(false);
    }
    let stations: Vec<Arc<Device>> = tx
        .iter::<Device>()
        .into_iter()
        .filter(|d| takes_cards(d))
        .collect();
    let station = match stations.as_slice() {
        [] => {
            tx.notice(text!("notice-no-station"));
            return Ok(false);
        }
        [one] => Arc::clone(one),
        several => match beside(tx, module, several) {
            Some(station) => station,
            None => {
                tx.notice(text!("notice-attach-station"));
                return Ok(false);
            }
        },
    };
    if is_at_or_above(tx, module, station.id) {
        tx.notice(text!("notice-attach-station"));
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
    tx.notice(text!(
        "notice-attached",
        ew_derive::text::letter_counter_tag(&station.tag),
        position.to_string()
    ));
    Ok(true)
}

/// After a station arrived, the modules that waited for one (no station ancestor, not integrated) attach to
/// the project's only station by D4, in the order of their main representations' anchor cells (ties by ID). With
/// several stations nothing happens here: EW-PLC-008 asks the user to attach them.
fn attach_waiting_modules(tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
    let stations = tx.iter::<Device>().into_iter().filter(|d| takes_cards(d)).count();
    if stations != 1 {
        return Ok(());
    }
    let waiting: Vec<Id<Device>> = tx
        .iter::<Device>()
        .into_iter()
        .filter(|d| {
            d.aspects.plc_station.is_none() && d.aspects.plc_module.as_ref().is_some_and(|m| !m.integrated)
        })
        .map(|d| d.id)
        .filter(|id| !has_station_ancestor(tx, *id))
        .collect();
    let mut keyed: Vec<_> = waiting
        .into_iter()
        .map(|id| (crate::numbering::place_key(tx, id), id))
        .collect();
    keyed.sort();
    for (_, module) in keyed {
        attach_module(tx, module)?;
    }
    Ok(())
}

impl Normaliser for PlcAspects {
    fn id(&self) -> &'static str {
        "plc_aspects"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        let mut stations: Vec<(Arc<Device>, PlcSection)> = Vec::new();
        let mut modules: Vec<Arc<Device>> = Vec::new();
        let mut candidates: std::collections::BTreeSet<Id<Device>> = tx
            .changed_of(Kind::Device)
            .into_iter()
            .map(|k| k.1.cast())
            .collect();
        candidates.extend(devices_of_changed_parts(tx));
        for id in candidates {
            let Some(dev) = tx.get::<Device>(id) else {
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
        let station_arrived = !stations.is_empty();
        for (dev, section) in stations {
            let role = station_role(tx, &dev, &section);
            let cfg = config_project_for(tx, &dev, &role)?;
            let integrated = (!section.io.is_empty()).then(|| PlcModule {
                integrated: true,
                ..PlcModule::default()
            });
            tx.update::<Device>(dev.id, |d| {
                d.aspects.plc_station = Some(PlcStation {
                    config_project: cfg,
                    role,
                    order: None,
                    firmware: None,
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
        if station_arrived {
            attach_waiting_modules(tx)?;
        }
        Ok(true)
    }
}
