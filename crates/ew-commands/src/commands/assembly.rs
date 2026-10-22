// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device tree of assemblies and valve terminals (doc 03 §7, doc 10 §3.7, doc 11 §3.2, §4.3; FT-169):
//! `assembly.attach` (`AttachChildren`), `valve.add` (`AddValve`) and `device.link` (`LinkDevices`).
//!
//! The typed `assembly`, `plc_station` and `plc_module` aspects are set with `fields.set`; adding a
//! `plc_module` instantiates the module's channels (normaliser N1, `channel_instantiation`).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Device, DeviceAspects, DeviceLink, DeviceLinkKind, Extra, FnDef, FnRole, Kind, LibCopy, PartUse,
    StructureAssignment, TagScheme, TagSpec,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::device::insert_function;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, UuidS};
use crate::tx::TxBuilder;

/// One child of [`AttachChildren`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChildSpec {
    /// The child device.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// Its position inside the parent (manifold order, 1-based); absent: no position (an unordered child
    /// such as a mounting plate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
}

/// The ancestors of `device` (its parent, the parent's parent, …), at most as many as there are devices.
fn ancestors(tx: &TxBuilder<'_>, device: Id<Device>) -> Vec<Id<Device>> {
    let mut out = Vec::new();
    let mut at = tx.get::<Device>(device).and_then(|d| d.parent);
    while let Some(p) = at {
        if out.contains(&p) {
            break; // a cycle the invariants report (EW-INV-009)
        }
        out.push(p);
        at = tx.get::<Device>(p).and_then(|d| d.parent);
    }
    out
}

/// The positions taken among the children of `parent`, except `skip`: position → child.
fn taken_positions(
    tx: &TxBuilder<'_>,
    parent: Id<Device>,
    skip: &BTreeSet<Id<Device>>,
) -> BTreeMap<u32, Id<Device>> {
    tx.iter::<Device>()
        .into_iter()
        .filter(|d| d.parent == Some(parent) && !skip.contains(&d.id))
        .filter_map(|d| d.position.map(|p| (p, d.id)))
        .collect()
}

/// `assembly.attach` (`AttachChildren`): makes existing devices children of a parent device (doc 03 §7:
/// modules of a valve terminal under its head device, a mounting plate under its enclosure), with their
/// positions (FR-DEV-022: child order is `Device.position`, never the drawing).
///
/// Sets `Device.parent = parent` and `Device.position` of every child (a child given without a position
/// keeps none). At least one child; children distinct and different from the parent; all exist
/// ([`CmdError::NotFound`]). Rejected: a position 0 (positions are 1-based, at `/children/i/position`); two
/// children of the command at one position; a position another child of the parent already holds (no
/// push insert in v0: move that child first); a child that is the parent's ancestor (the assembly graph is
/// acyclic, EW-INV-009). A child that has another parent moves to this one. A child tag (`tag.scheme =
/// child`, not frozen) whose counter equals its old position follows the new position (N-FL-2, doc 11
/// §4.2); any other tag is not touched. A child tag is set with `fields.set`, a valve added with `valve.add`.
/// The generated assembly overviews, slots, break marks and the PLC overview follow from the tree (they are
/// derived). Outcome: the parent.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachChildren {
    /// The parent device.
    #[schemars(with = "UuidS")]
    pub parent: Id<Device>,
    /// The children, at least one.
    pub children: Vec<ChildSpec>,
}

impl Command for AttachChildren {
    const NAME: &'static str = "assembly.attach";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.children.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/children",
                "give at least one child",
            ));
        }
        tx.require::<Device>(self.parent)?;
        let mut seen = BTreeSet::new();
        let mut positions: BTreeMap<u32, Id<Device>> = BTreeMap::new();
        for (i, c) in self.children.iter().enumerate() {
            if !seen.insert(c.device) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/device"),
                    "the child is listed twice",
                ));
            }
            if c.device == self.parent {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/device"),
                    "a device cannot be its own child",
                ));
            }
            if c.position == Some(0) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/children/{i}/position"),
                    "positions are 1-based",
                ));
            }
            tx.require::<Device>(c.device)?;
            if let Some(p) = c.position {
                if let Some(other) = positions.insert(p, c.device) {
                    return tx.reject(format!(
                        "position {p} is given to two children ({other} and {})",
                        c.device
                    ));
                }
            }
        }
        if let Some(c) = ancestors(tx, self.parent).into_iter().find(|a| seen.contains(a)) {
            return tx.reject(format!(
                "device {c} is an ancestor of the parent {}; the assembly graph is acyclic (EW-INV-009)",
                self.parent
            ));
        }
        let taken = taken_positions(tx, self.parent, &seen);
        for (p, child) in &positions {
            if let Some(holder) = taken.get(p) {
                return tx.reject(format!(
                    "position {p} of {} is held by device {holder}; move it first (no push insert), then \
                     attach {child}",
                    self.parent
                ));
            }
        }
        for c in &self.children {
            let (parent, position) = (self.parent, c.position);
            tx.update::<Device>(c.device, |d| {
                // N-FL-2 (doc 11 §3.2, §4.2): a child tag keeps its counter equal to its position; a frozen
                // tag or a counter that differed from the old position is the user's and stays
                let follows = d.tag.scheme == TagScheme::Child
                    && d.tag.frozen.is_none()
                    && d.tag.counter.is_some()
                    && d.tag.counter == d.position;
                if follows && position.is_some() {
                    d.tag.counter = position;
                }
                d.parent = Some(parent);
                d.position = position;
            })?;
        }
        Ok(Outcome::primary((Kind::Device, self.parent.cast())))
    }
}

/// `valve.add` (`AddValve`, doc 11 §4.3 `valve.add{module, position?, part}`): adds a valve child to a
/// module (sub-base) of a valve terminal.
///
/// Creates `Device {tag: {letter, counter: position, scheme: child}, parent: module, position, structure,
/// parts: [part]}` (doc 11 §3.2: the child rule prints `{parent}-{letter}{position}`; normaliser N-FL-2
/// keeps the counter equal to the position) and, with `fndef`, its main function `{def: fndef, role: main,
/// kind: normal}`. The position is the requested one, else the lowest free position of the module (1, 2,
/// …). The module, the part copy and the FnDef copy must exist. Rejected: position 0 (at `/position`); a
/// requested position another child of the module holds (no push insert in v0). Not in v0: the coil
/// functions from the part's `solenoids` (N-FL-1; give `fndef`), the slot-count bound of the module's part
/// (EW-FL-003 is a check) and the coil-to-channel binding by the part's position map (N2: bind with
/// `plc.bind_channel`). The state (spare valves) and the function text are set with `fields.set`.
/// Outcome: the valve.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddValve {
    /// The module (sub-base) the valve sits on.
    #[schemars(with = "UuidS")]
    pub module: Id<Device>,
    /// The requested position (1-based); default the lowest free one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// The valve's part use.
    pub part: PartUse,
    /// The letter code; default `Q` (directional valves in the shipped EN 81346-2:2009 set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub letter: Option<Ident>,
    /// The FnDef copy of the valve's main function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub fndef: Option<Id<FnDef>>,
    /// Own structure assignment; default none (inherited from the parent).
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
}

impl Command for AddValve {
    const NAME: &'static str = "valve.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.position == Some(0) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/position",
                "positions are 1-based",
            ));
        }
        tx.require::<Device>(self.module)?;
        tx.require::<LibCopy<ew_schema::Part>>(self.part.part.copy)?;
        if let Some(fndef) = self.fndef {
            tx.require::<LibCopy<FnDef>>(fndef)?;
        }
        let taken = taken_positions(tx, self.module, &BTreeSet::new());
        let position = match self.position {
            Some(p) => {
                if let Some(holder) = taken.get(&p) {
                    return tx.reject(format!(
                        "position {p} of module {} is held by device {holder}; choose a free position (no \
                         push insert)",
                        self.module
                    ));
                }
                p
            }
            None => (1..).find(|p| !taken.contains_key(p)).unwrap_or(u32::MAX),
        };
        let letter = self
            .letter
            .clone()
            .unwrap_or_else(|| "Q".parse().expect("`Q` is an identifier"));
        let id: Id<Device> = tx.mint();
        tx.insert(Device {
            id,
            v: 1,
            tag: TagSpec {
                scheme: TagScheme::Child,
                ..TagSpec::new(letter, Some(position))
            },
            kind: None,
            parent: Some(self.module),
            position: Some(position),
            structure: self.structure.clone(),
            parts: vec![self.part.clone()],
            function_text: MlText::default(),
            process_tag: None,
            tech_data: Default::default(),
            state: Default::default(),
            source_ref: None,
            aspects: DeviceAspects::default(),
            extra: Extra::new(),
        })?;
        if let Some(def) = self.fndef {
            insert_function(tx, Some(id), def, FnRole::Main)?;
        }
        Ok(Outcome::primary((Kind::Device, id.cast())))
    }
}

/// `device.link` (`LinkDevices`, doc 11 §4.3 `device.link{from, to, Drives}`): a functional link between
/// two devices, such as a valve that drives a field device (doc 11 §3.3: the PLC overview and the I/O
/// list print the driven device's process tag).
///
/// Creates `DeviceLink {from, to, kind}`. Both devices exist ([`CmdError::NotFound`]); rejected: `from =
/// to` (at `/to`), and a link of the same kind between the same devices that exists already. Deleting
/// either device deletes the link (cascade). Outcome: the link.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkDevices {
    /// The driving or linked device.
    #[schemars(with = "UuidS")]
    pub from: Id<Device>,
    /// The driven or linked device.
    #[schemars(with = "UuidS")]
    pub to: Id<Device>,
    /// The relation.
    pub kind: DeviceLinkKind,
}

impl Command for LinkDevices {
    const NAME: &'static str = "device.link";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.from == self.to {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/to",
                "a device cannot link to itself",
            ));
        }
        tx.require::<Device>(self.from)?;
        tx.require::<Device>(self.to)?;
        if let Some(l) = tx
            .iter::<DeviceLink>()
            .into_iter()
            .find(|l| l.from == self.from && l.to == self.to && l.kind == self.kind)
        {
            return tx.reject(format!(
                "device {} already links to {} with this kind (link {})",
                self.from, self.to, l.id
            ));
        }
        let id: Id<DeviceLink> = tx.mint();
        tx.insert(DeviceLink {
            id,
            v: 1,
            from: self.from,
            to: self.to,
            kind: self.kind.clone(),
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::DeviceLink, id.cast())))
    }
}
