// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The v0 editor commands (doc 06 §3.1, within the v0 record set). Every command struct is its own
//! parameter struct, with serde, a JSON Schema and `deny_unknown_fields`.
//!
//! | Name | Struct | Cost |
//! |---|---|---|
//! | `page.create` | [`CreatePage`] | Interactive |
//! | `page.delete` | [`DeletePages`] | Bulk |
//! | `page.renumber` | [`RenumberPages`] | Bulk |
//! | `structure.create_node` | [`CreateStructureNode`] | Interactive |
//! | `section.create` | [`CreateSection`] | Interactive |
//! | `report_block.create` | [`CreateReportBlock`] | Interactive |
//! | `device.create` | [`CreateDevice`] | Interactive |
//! | `device.place` | [`PlaceSymbol`] | Interactive |
//! | `rep.move` | [`MoveReps`] | Interactive |
//! | `rep.transform` | [`TransformReps`] | Interactive |
//! | `rep.set_anchor_policy` | [`SetAnchorPolicy`] | Interactive |
//! | `connection.draw` | [`Connect`] | Interactive |
//! | `connection.route` | [`RouteConnection`] | Interactive |
//! | `connection.delete` | [`DeleteConnections`] | Interactive |
//! | `junction.edit` | [`EditJunction`] | Interactive |
//! | `ip.place` | [`PlaceIp`] | Interactive |
//! | `ip.continue` | [`ContinueIp`] | Interactive |
//! | `potential.define` | [`DefinePotential`] | Interactive |
//! | `terminal.add` | [`AddTerminals`] | Interactive |
//! | `terminal.designate` | [`DesignateTerminals`] | Interactive |
//! | `cable.create` | [`CreateCable`] | Interactive |
//! | `conductor.add` | [`AddConductors`] | Interactive |
//! | `fields.set` | [`SetFields`] | Interactive |
//! | `edit.delete` | [`DeleteSelection`] | Interactive; Bulk above 500 targets |
//! | `numbering.renumber` | [`RenumberDevices`] | Bulk |
//! | `project.settings.set` | [`SetProjectSettings`] | Bulk |
//!
//! Every command is undoable, of exposure class `Edit` and stability `Preview`.

pub mod common;
mod connection;
mod device;
mod edit;
mod fields;
mod page;
mod renumber;
mod settings;
mod structure;
mod terminal;

pub use common::{EndpointParam, KindName, TargetRef};
pub use connection::{
    Connect, ContinueIp, DefinePotential, DeleteConnections, EditJunction, PlaceIp, RouteConnection,
    WiringChoice,
};
pub use device::{
    ConnectHint, CreateDevice, MoveReps, PlaceSymbol, PlaceTarget, SetAnchorPolicy, TransformReps,
};
pub use edit::{BULK_ABOVE, DeleteSelection};
pub use fields::{ASPECTS, SETTABLE, SetFields};
pub use page::{CreatePage, DeletePages, PageScope, RenumberPages};
pub use renumber::{DeviceScope, NumberingScheme, RenumberDevices, RenumberOptions};
pub use settings::{SETTABLE_SETTINGS, SetProjectSettings, SettableSetting};
pub use structure::{CreateReportBlock, CreateSection, CreateStructureNode};
pub use terminal::{
    AddConductors, AddTerminals, ConductorSpec, CreateCable, DesignateTerminals, TerminalDesignation,
    TerminalSpec,
};

use crate::registry::CommandRegistry;

/// Registers every built-in command: the single list of commands.
pub fn register_all(r: &mut CommandRegistry) {
    r.register::<CreatePage>();
    r.register::<DeletePages>();
    r.register::<RenumberPages>();
    r.register::<CreateStructureNode>();
    r.register::<CreateSection>();
    r.register::<CreateReportBlock>();
    r.register::<CreateDevice>();
    r.register::<PlaceSymbol>();
    r.register::<MoveReps>();
    r.register::<TransformReps>();
    r.register::<SetAnchorPolicy>();
    r.register::<Connect>();
    r.register::<RouteConnection>();
    r.register::<DeleteConnections>();
    r.register::<EditJunction>();
    r.register::<PlaceIp>();
    r.register::<ContinueIp>();
    r.register::<DefinePotential>();
    r.register::<AddTerminals>();
    r.register::<DesignateTerminals>();
    r.register::<CreateCable>();
    r.register::<AddConductors>();
    r.register::<SetFields>();
    r.register::<DeleteSelection>();
    r.register::<RenumberDevices>();
    r.register::<SetProjectSettings>();
}
