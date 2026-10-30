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
//! | `page.reorder` | [`ReorderPages`] | Bulk |
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
//! | `box.black` | [`PlaceBlackBox`] | Interactive |
//! | `bond.create` | [`CreateBond`] | Interactive |
//! | `external_ref.create` | [`CreateExternalRef`] | Interactive |
//! | `assembly.attach` | [`AttachChildren`] | Interactive |
//! | `valve.add` | [`AddValve`] | Interactive |
//! | `device.link` | [`LinkDevices`] | Interactive |
//! | `plc.config_project.create` | [`CreatePlcConfigProject`] | Interactive |
//! | `plc.bind_channel` | [`BindChannel`] | Interactive |
//! | `plc.set_module_part` | [`SetModulePart`] | Bulk |
//! | `project.supply.set` | [`SetProjectSupply`] | Interactive |
//! | `library.embed` | [`EmbedLibraryCopies`] | Interactive |
//! | `revision.create` | [`CreateRevision`] | Interactive |
//! | `revision.set_author` | [`SetRevisionAuthor`] | Interactive; exposure `HumanOnly` |
//! | `revision.withdraw` | [`WithdrawRevision`] | Interactive; exposure `HumanOnly` |
//! | `project.company.set` | [`SetCompany`] | Bulk |
//! | `project.frames.set` | [`SetFrameDefaults`] | Bulk |
//! | `project.languages.set` | [`SetProjectLanguages`] | Bulk |
//! | `project.checks.set` | [`SetCheckSchemes`] | Interactive; exposure `HumanOnly` |
//! | `check.waive` | [`WaiveFinding`] | Interactive; exposure `HumanOnly` |
//! | `check.unwaive` | [`UnwaiveFinding`] | Interactive; exposure `HumanOnly` |
//! | `project.assets.purge` | [`PurgeAssets`] | Bulk; not undoable (a marker line), confirmation `Destructive`; exposure `HumanOnly` |
//! | `rep.exchange_symbol` | [`ExchangeSymbol`] | Interactive |
//! | `project.drawing.set` | [`SetDrawing`] | Interactive |
//! | `fields.set_many` | [`SetManyFields`] | Interactive; Bulk above 500 targets |
//! | `edit.paste` | [`PasteFragment`] | Interactive; Bulk above 500 records |
//! | `connection.draw_to_ip` | [`DrawToIp`] | Interactive |
//! | `ip.arrange` | [`ArrangeIps`] | Interactive |
//! | `box.reshape` | [`ReshapeBox`] | Interactive |
//! | `box.structure` | [`CreateStructureBox`] | Interactive |
//! | `item.place` | [`PlaceItem`] | Interactive |
//! | `marker.place` | [`PlaceMarker`] | Interactive |
//! | `parts.import` | [`ImportParts`] | Bulk |
//! | `connection.draw_many` | [`ConnectMany`] | Interactive; Bulk above 500 entries |
//! | `terminal.insert` | [`InsertTerminals`] | Interactive |
//! | `terminal.move` | [`MoveTerminals`] | Interactive |
//! | `part.assign` | [`AssignPart`] | Interactive; Bulk from 50 devices |
//! | `library.update` | [`UpdateLibraryCopies`] | Bulk |
//! | `macro.insert` | [`InsertMacro`] | Interactive |
//! | `rep.join_device` | [`JoinDevice`] | Interactive |
//!
//! Every command is of stability `Preview` and undoable except `project.assets.purge`, a marker line whose file side
//! (deleting asset files nothing names) runs in the session (ADR-0008 amendment 1, FT-380); every one is of exposure
//! class `Edit` except `project.checks.set`, `check.waive` and `check.unwaive`, which are `HumanOnly` (ADR-0008 amendment
//! 3: they decide a check outcome), `revision.set_author` and `revision.withdraw`, which are `HumanOnly` because they
//! decide the four-eyes sign-off and the release history (WP-S1.18 D4, D5),
//! and `project.assets.purge`, which is `HumanOnly` because deleted files do not come back (owner ruling, 2026-10-23).

mod assembly;
mod assets;
mod assign;
mod box_reshape;
mod checks;
pub mod common;
mod company;
mod connection;
mod device;
mod drawing;
mod edit;
mod exchange;
mod fields;
mod frames;
pub(crate) mod items;
mod join;
mod languages;
pub(crate) mod library;
mod library_update;
// WP-S1.19 part 5
mod macro_insert;
mod page;
// WP-S2.05 part 3
mod parts;
mod paste;
mod plc;
mod renumber;
mod revision;
mod settings;
mod structure;
pub(crate) mod structure_box;
mod supply;
mod terminal;

pub use assembly::{AddValve, AttachChildren, ChildSpec, LinkDevices};
pub use assets::PurgeAssets;
pub use assign::{ASSIGN_BULK_FROM, AssignModeParam, AssignPart};
pub use box_reshape::ReshapeBox;
pub use checks::{
    CHECKS_MEMBER, SHIPPED_CHECK_SCHEMES, SetCheckSchemes, UnwaiveFinding, WAIVER_REASON_MIN_CHARS,
    WaiveFinding,
};
pub use common::{EndpointParam, KindName, TargetRef};
pub use company::{COMPANY_MEMBER, SetCompany};
pub use connection::{
    ArrangeIps, Connect, ConnectMany, ContinueIp, CreateBond, DefinePotential, DeleteConnections, DrawToIp,
    EditJunction, IpAtEnd, PlaceIp, RouteConnection, WiringChoice,
};
pub use device::{
    ConnectHint, CreateDevice, MoveReps, PlaceBlackBox, PlaceSymbol, PlaceTarget, SetAnchorPolicy,
    TransformReps, allowed_rep_types,
};
pub use drawing::{Crossings, DRAWING_MEMBER, MarkerModeParam, SetDrawing};
pub use edit::{BULK_ABOVE, DeleteSelection};
pub use exchange::{ExchangePin, ExchangeSymbol, PinPairs, derive_pin_map};
pub use fields::{ASPECTS, SET_MANY_MAX_EDITS, SETTABLE, SetFields, SetManyFields};
pub use frames::{FRAMES_MEMBER, FrameForPageType, SetFrameDefaults};
pub use items::{
    AlignParam, DashParam, FreeTextParam, GraphicParam, ImageParam, ItemKindParam, PlaceItem, PlaceMarker,
    ShapeParam, StrokeParam,
};
pub use join::JoinDevice;
pub use languages::SetProjectLanguages;
pub use library::{EmbedLibraryCopies, library_copy_hash};
pub use library_update::{
    ChangeClass, CopyDiff, CpMapParam, FieldChange, UpdateLibraryCopies, classify_update,
};
pub use macro_insert::{
    InsertMacro, InstantiateError, Instantiated, MACRO_FIELDS, MACRO_KINDS, binding_field_ok, coerce_value,
    instantiate, pick_variant,
};
pub use page::{CreatePage, DeletePages, PageScope, RenumberPages, ReorderPages};
// WP-S2.05 part 3
pub use parts::{ImportParts, import_counts};
pub use paste::{Fragment, FragmentNode, PASTE_KINDS, PASTE_MAX_RECORDS, PasteFragment};
pub use plc::{BindChannel, CreatePlcConfigProject, SetModulePart};
pub use renumber::{DeviceScope, NumberingScheme, RenumberDevices, RenumberOptions};
pub use revision::{
    CreateRevision, LABEL_ORDER_RULE, NewRevisionState, RevisionPageNote, SetRevisionAuthor, WithdrawRevision,
};
pub use settings::{SETTABLE_SETTINGS, SetProjectSettings, SettableSetting};
pub use structure::{CreateExternalRef, CreateReportBlock, CreateSection, CreateStructureNode};
pub use structure_box::{BoxStyleParam, CreateStructureBox};
pub use supply::{SETTABLE_SUPPLY, SetProjectSupply};
pub use terminal::{
    AddConductors, AddTerminals, ConductorSpec, CreateCable, DesignateTerminals, InsertTerminals,
    MoveTerminals, TerminalDesignation, TerminalSpec,
};

use crate::registry::CommandRegistry;

/// Registers every built-in command: the single list of commands.
pub fn register_all(r: &mut CommandRegistry) {
    r.register::<CreatePage>();
    r.register::<DeletePages>();
    r.register::<RenumberPages>();
    r.register::<ReorderPages>();
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
    r.register::<PlaceBlackBox>();
    r.register::<CreateBond>();
    r.register::<CreateExternalRef>();
    r.register::<AttachChildren>();
    r.register::<AddValve>();
    r.register::<LinkDevices>();
    r.register::<CreatePlcConfigProject>();
    r.register::<BindChannel>();
    r.register::<SetModulePart>();
    r.register::<SetProjectSupply>();
    r.register::<EmbedLibraryCopies>();
    r.register::<CreateRevision>();
    r.register::<SetRevisionAuthor>();
    r.register::<WithdrawRevision>();
    r.register::<SetCompany>();
    r.register::<SetFrameDefaults>();
    r.register::<SetProjectLanguages>();
    r.register::<SetCheckSchemes>();
    r.register::<PurgeAssets>();
    r.register::<ExchangeSymbol>();
    r.register::<SetDrawing>();
    // WP-S2.05 part 2
    r.register::<SetManyFields>();
    // WP-S2.05 part 3
    r.register::<PasteFragment>();
    // WP-S2.04 part 3
    r.register::<DrawToIp>();
    r.register::<ArrangeIps>();
    r.register::<ReshapeBox>();
    // WP-S2.04 part 4a
    r.register::<CreateStructureBox>();
    r.register::<PlaceItem>();
    r.register::<PlaceMarker>();
    // FT-410
    r.register::<ImportParts>();
    // Cycle 33: the chain of bridges of Edit ▸ Bridge terminals
    r.register::<ConnectMany>();
    // WP-S2.07 part 1: the strip editor's insert and move
    r.register::<InsertTerminals>();
    r.register::<MoveTerminals>();
    // WP-S1.19 part 3
    r.register::<AssignPart>();
    r.register::<UpdateLibraryCopies>();
    // WP-S1.19 part 5
    r.register::<InsertMacro>();
    // WP-S1.16 part 5: waiver recording (D7, FT-494)
    r.register::<WaiveFinding>();
    r.register::<UnwaiveFinding>();
    // WP-S2.18 D3 (FT-628)
    r.register::<JoinDevice>();
}
