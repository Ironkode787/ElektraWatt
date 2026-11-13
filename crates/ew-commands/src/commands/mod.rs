// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The v0 editor commands (doc 06 §3.1, within the v0 record set). Every command struct is its own
//! parameter struct, with serde, a JSON Schema and `deny_unknown_fields`.
//!
//! The commands are listed once, in the `commands!` invocation below; [`CommandRegistry::builtin`] holds every one by
//! name. The book's command table (`docs/book/src/reference/commands/index.md`) is generated from their schemas.
//!
//! Every command is of stability `Preview` and undoable except `project.assets.purge`, a marker line whose file side
//! (deleting asset files nothing names) runs in the session (ADR-0008 amendment 1), and `revision.release`,
//! the release's stamp, which a released revision's immutability makes a barrier (FR-REV-009); every one is of exposure
//! class `Edit` except `project.checks.set`, `check.waive` and `check.unwaive`, which are `HumanOnly` (ADR-0008 amendment
//! 3: they decide a check outcome), `revision.set_author`, `revision.withdraw`, `revision.approve` and
//! `revision.release`, which are `HumanOnly` because they decide the four-eyes sign-off and the release history,
//! and `project.assets.purge`, which is `HumanOnly` because deleted files do not come back.
//! `external.apply` is of exposure class `Internal` and stability `Internal`: only the session's external-change apply
//! submits it, from the origin `external`.

mod assembly;
mod assets;
mod assign;
mod box_reshape;
mod bus;
mod checks;
pub mod common;
mod company;
mod connection;
mod device;
mod drawing;
mod edit;
mod exchange;
mod external;
mod fields;
mod fluid;
mod frames;
mod import;
pub(crate) mod items;
mod join;
mod languages;
mod layout;
pub(crate) mod library;
mod library_update;
mod macro_insert;
mod merge;
mod page;
mod part_edit;
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

pub use assembly::ChildSpec;
pub use assign::AssignModeParam;
pub use checks::{CHECKS_MEMBER, SHIPPED_CHECK_SCHEMES};
pub use common::{EndpointParam, KindName, TargetRef};
pub use company::COMPANY_MEMBER;
pub use connection::{IpAtEnd, WiringChoice};
pub use device::{ConnectHint, PlaceTarget, allowed_rep_types};
pub use drawing::{Crossings, MarkerModeParam};
pub use edit::BULK_ABOVE;
pub use exchange::{ExchangePin, derive_pin_map};
pub use fields::{ASPECTS, SET_MANY_MAX_EDITS, SETTABLE};
pub use fluid::{ClosureKindParam, ClosureParam, MediumParam, OccupancyParam, TubeField, ValveStateParam};
pub use frames::FRAMES_MEMBER;
pub use import::{
    ApplyImport, MemberChange, ReimportClass, ReimportPlan, ReimportRow, lineage_keys, member_text,
    name_rows, plan_reimport,
};
pub use items::{
    AlignParam, DashParam, FreeTextParam, GraphicParam, ImageParam, ItemKindParam, ShapeParam, StrokeParam,
};
pub use layout::{CarrierKindParam, CarrierParamsParam, CarrierThen, FillOrder, PlaceMode, PlaceOnParam};
pub use library::library_copy_hash;
pub use library_update::{ChangeClass, CopyDiff, CpMapParam, FieldChange, classify_update};
pub use macro_insert::{MACRO_KINDS, binding_field_ok, coerce_value, instantiate};
pub use merge::Take;
#[cfg(feature = "testkit")]
pub use merge::record_file_of;
pub use page::{LabelModeParam, PageScope};
pub use parts::import_counts;
pub use paste::{Fragment, FragmentNode, PASTE_KINDS, PASTE_MAX_RECORDS};
pub use plc::is_on_module;
pub use renumber::{DeviceScope, NumberingScheme, RenumberOptions};
pub use revision::{ApprovedPack, LABEL_ORDER_RULE, NewRevisionState, ReleasedArchive, RevisionPageNote};
pub use settings::SETTABLE_SETTINGS;
pub use structure_box::BoxStyleParam;
pub use supply::SETTABLE_SUPPLY;
pub use terminal::{ConductorSpec, TerminalDesignation, TerminalSpec};

use crate::registry::CommandRegistry;

/// Declares the built-in commands once: every `module::Type` entry becomes a `pub use` of the command type and one
/// `register` call in [`register_all`].
macro_rules! commands {
    ($($module:ident::$ty:ident),* $(,)?) => {
        $(pub use $module::$ty;)*

        /// Registers every built-in command: the single list of commands.
        pub fn register_all(r: &mut CommandRegistry) {
            $(r.register::<$ty>();)*
        }
    };
}

commands! {
    page::CreatePage,
    page::DeletePages,
    page::RenumberPages,
    page::ReorderPages,
    page::SetSurfaceView,
    structure::CreateStructureNode,
    structure::CreateSection,
    structure::CreateReportBlock,
    structure::CreateExternalRef,
    structure::MarkSupplyIn,
    device::CreateDevice,
    device::PlaceSymbol,
    device::MoveReps,
    device::TransformReps,
    device::SetAnchorPolicy,
    device::PlaceBlackBox,
    connection::Connect,
    connection::RouteConnection,
    connection::DeleteConnections,
    connection::EditJunction,
    connection::PlaceIp,
    connection::ContinueIp,
    connection::DefinePotential,
    connection::CreateBond,
    connection::DrawToIp,
    connection::ArrangeIps,
    // The chain of bridges of Edit ▸ Bridge terminals
    connection::ConnectMany,
    // A page's or a selection's routes in one step
    connection::RouteMany,
    terminal::AddTerminals,
    terminal::DesignateTerminals,
    terminal::CreateCable,
    terminal::AddConductors,
    // The strip editor's insert and move
    terminal::InsertTerminals,
    terminal::MoveTerminals,
    fields::SetFields,
    fields::SetManyFields,
    edit::DeleteSelection,
    renumber::RenumberDevices,
    settings::SetProjectSettings,
    assembly::AttachChildren,
    assembly::AddValve,
    assembly::LinkDevices,
    fluid::SetValveOccupancy,
    fluid::ClosePort,
    fluid::EditMedium,
    fluid::DefineMedium,
    fluid::SetTube,
    fluid::SetLength,
    plc::CreatePlcConfigProject,
    plc::BindChannel,
    plc::SetModulePart,
    plc::SetStart,
    plc::SetProfile,
    plc::FreezeAddresses,
    plc::Readdress,
    plc::ConvertManualToAuto,
    bus::CreateLogicalNet,
    bus::CreatePhysicalNet,
    supply::SetProjectSupply,
    library::EmbedLibraryCopies,
    revision::CreateRevision,
    revision::SetRevisionAuthor,
    revision::WithdrawRevision,
    // The sign-off (D9)
    revision::ApproveRevision,
    // The release's stamp (D13)
    revision::ReleaseRevision,
    company::SetCompany,
    frames::SetFrameDefaults,
    languages::SetProjectLanguages,
    checks::SetCheckSchemes,
    // Waiver recording (D7)
    checks::WaiveFinding,
    checks::UnwaiveFinding,
    // Conflict records (D19)
    merge::ResolveConflicts,
    // An external change (D22)
    external::ApplyExternal,
    assets::PurgeAssets,
    exchange::ExchangeSymbol,
    drawing::SetDrawing,
    paste::PasteFragment,
    box_reshape::ReshapeBox,
    structure_box::CreateStructureBox,
    items::PlaceItem,
    items::PlaceMarker,
    parts::ImportParts,
    assign::AssignPart,
    library_update::UpdateLibraryCopies,
    macro_insert::InsertMacro,
    join::JoinDevice,
    part_edit::EditPartCopy,
    layout::CreateLayoutSpace,
    layout::DeleteLayoutSpace,
    layout::CreateEnclosure,
    layout::DeleteEnclosure,
    layout::AddCarrier,
    layout::EditCarrier,
    layout::DeleteCarrier,
    layout::PlaceDevice,
    layout::MovePlacement,
    layout::RemovePlacement,
    layout::LockPlacement,
    layout::PlaceAssembly,
    layout::AutoFill,
    layout::CreateEnclosureFromPart,
    layout::EditLayoutSpace,
    layout::EditEnclosure,
    layout::EditSurface,
}
