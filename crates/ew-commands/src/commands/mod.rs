// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The v0 editor commands (doc 06 §3.1, within the v0 record set). Every command struct is its own
//! parameter struct, with serde, a JSON Schema and `deny_unknown_fields`.
//!
//! The commands are listed once, in the `commands!` invocation below; [`CommandRegistry::builtin`] holds every one by
//! name. The book's command table (`docs/book/src/reference/commands/index.md`) is generated from their schemas.
//!
//! Every command is undoable except `project.assets.purge`, a marker line whose file side
//! (deleting asset files nothing names) runs in the session (ADR-0008 amendment 1), and `revision.release`,
//! the release's stamp, which a released revision's immutability makes a barrier (FR-REV-009); every one is of exposure
//! class `Edit` except `project.checks.set`, `check.waive` and `check.unwaive`, which are `HumanOnly` (ADR-0008 amendment
//! 3: they decide a check outcome), `revision.set_author`, `revision.withdraw`, `revision.approve` and
//! `revision.release`, which are `HumanOnly` because they decide the four-eyes sign-off and the release history,
//! and `project.assets.purge`, which is `HumanOnly` because deleted files do not come back.
//! `external.apply` is of exposure class `Internal`: only the session's external-change apply submits it, from the
//! origin `external`.

mod assembly;
mod assets;
mod assign;
mod box_follow;
mod box_reshape;
mod bus;
mod cable_mark;
mod checks;
pub mod common;
mod company;
mod connection;
mod device;
mod drafting;
mod drawing;
mod edit;
mod exchange;
mod external;
mod face_card;
mod fields;
mod fluid;
mod form_edit;
mod frames;
mod frames_adopt;
mod import;
pub(crate) mod items;
mod join;
mod join_level;
mod languages;
mod layout;
pub(crate) mod library;
mod library_update;
mod macro_insert;
mod merge;
mod move_keep;
mod overview;
mod page;
mod page_copy;
mod part_delete;
mod part_edit;
mod parts;
mod paste;
mod placeholder;
mod plc;
mod plc_exchange;
mod renumber;
mod revision;
mod settings;
mod structure;
pub(crate) mod structure_box;
mod supply;
mod terminal;
mod typeface;
mod user_prop;
mod workshop;

pub use assembly::ChildSpec;
pub use assign::AssignModeParam;
pub use bus::{BusLineField, PlugParam};
pub use common::{EndpointParam, KindName, TargetRef};
pub use connection::{IpAtEnd, WiringChoice, next_in_series};
pub use device::{ConnectHint, PlaceTarget, PortBox, PortsTarget, allowed_rep_types};
pub use drafting::{JunctionPlace, MarkerPlace};
pub use edit::BULK_ABOVE;
pub use exchange::{ExchangePin, derive_pin_map};
pub use fields::{ASPECTS, SET_MANY_MAX_EDITS, SETTABLE};
pub use fluid::{ClosureKindParam, ClosureParam, MediumParam, OccupancyParam, TubeField, ValveStateParam};
pub use form_edit::{FieldFromCell, GridAxis, MAX_AXIS, MAX_FIRST, MIN_CELL, PictureInCell, forked_copies};
pub use frames::FRAMES_MEMBER;
pub use import::{
    ApplyImport, MemberChange, ReimportClass, ReimportPlan, ReimportRow, lineage_keys, member_text,
    name_rows, plan_reimport,
};
pub use items::{
    AlignParam, AnchorParam, DashParam, FreeTextParam, GraphicParam, ImageParam, ItemKindParam, ShapeParam,
    StrokeParam,
};
pub use join::JoinClass;
pub use layout::{CarrierKindParam, CarrierParamsParam, CarrierThen, FillOrder, PlaceMode, PlaceOnParam};
pub use library_update::{ChangeClass, CopyDiff, CpMapParam, FieldChange, classify_update};
pub use macro_insert::{MACRO_KINDS, binding_field_ok, coerce_value, instantiate};
pub use merge::Take;
#[cfg(debug_assertions)]
pub use merge::record_file_of;
pub use move_keep::KeepAsIps;
pub use page::{LabelModeParam, LabelsParam, PageScope};
pub use page_copy::{
    NodeSwap, PAGE_COPY_KINDS, REPRODUCED, Rename, TextNames, designation_moves, names_tag, renamed,
    renamed_mltext, text_names,
};

pub use paste::{Fragment, FragmentNode, PASTE_KINDS, PASTE_MAX_RECORDS};
pub use plc::is_on_module;
pub use plc_exchange::{field_text, object_text};
pub use renumber::{DeviceScope, NumberingScheme, RenumberOptions};
pub use revision::{ApprovedPack, LABEL_ORDER_RULE, NewRevisionState, ReleasedArchive, RevisionPageNote};
pub use settings::SETTABLE_SETTINGS;
pub use structure_box::BoxStyleParam;
pub use supply::SETTABLE_SUPPLY;
pub use terminal::{ConductorSpec, TerminalDesignation, TerminalSpec};
pub use user_prop::{mint_user_prop_key, user_prop_path};
pub use workshop::{SETTABLE_WORKSHOP, WORKSHOP_PREFIX};

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
    page::SetPageGrid,
    // A copy of pages for a second installation
    page_copy::CopyPages,
    structure::CreateStructureNode,
    structure::CreateSection,
    structure::CreateReportBlock,
    structure::CreateExternalRef,
    structure::MarkSupplyIn,
    device::CreateDevice,
    device::PlaceSymbol,
    device::MoveReps,
    device::MoveRepsToNewPage,
    device::TransformReps,
    device::SetAnchorPolicy,
    device::SetRepText,
    device::ResetRepTexts,
    device::PlaceBlackBox,
    device::PlacePortBoxes,
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
    // The plan of Route in one step
    drafting::ApplyLayout,
    terminal::AddTerminals,
    terminal::DesignateTerminals,
    terminal::CreateCable,
    terminal::AddConductors,
    // The strip editor's insert and move
    terminal::InsertTerminals,
    terminal::MoveTerminals,
    // The wire to a card on the terminal's pin that faces it
    face_card::FaceCard,
    fields::SetFields,
    fields::SetManyFields,
    edit::DeleteSelection,
    renumber::RenumberDevices,
    settings::SetProjectSettings,
    assembly::AttachChildren,
    assembly::AddValve,
    assembly::LinkDevices,
    // A valve terminal's free box redrawn as its positions
    assembly::DrawSlots,
    // Draw its valves in the terminal
    assembly::DrawValves,
    fluid::SetValveOccupancy,
    fluid::ClosePort,
    fluid::EditMedium,
    fluid::DefineMedium,
    fluid::SetTube,
    fluid::SetLength,
    overview::GenerateOverview,
    plc::CreatePlcConfigProject,
    plc::BindChannel,
    plc::SetModulePart,
    plc::SetStart,
    plc::SetProfile,
    plc::FreezeAddresses,
    plc::Readdress,
    plc::ConvertManualToAuto,
    // The rack and slot editor (FR-PLC-028)
    plc::AddModule,
    plc::RemoveFromRack,
    plc::BindPins,
    plc::AddStation,
    plc::DrawStation,
    plc_exchange::RecordExchange,
    plc_exchange::ApplyExchange,
    plc_exchange::Unlock,
    plc_exchange::ApplyInstrumentList,
    bus::CreateLogicalNet,
    bus::CreatePhysicalNet,
    bus::SetBusCable,
    library::EmbedLibraryCopies,
    revision::CreateRevision,
    revision::SetRevisionAuthor,
    revision::WithdrawRevision,
    // The sign-off
    revision::ApproveRevision,
    // The release's stamp
    revision::ReleaseRevision,
    company::SetCompany,
    typeface::SetTypeface,
    checks::SetCheckSchemes,
    // Waiver recording
    checks::WaiveFinding,
    checks::FixAll,
    checks::UnwaiveFinding,
    // Conflict records
    merge::ResolveConflicts,
    // An external change
    external::ApplyExternal,
    assets::PurgeAssets,
    exchange::ExchangeSymbol,
    paste::PasteFragment,
    box_reshape::ReshapeBox,
    box_reshape::RedrawBoxes,
    structure_box::CreateStructureBox,
    items::PlaceItem,
    items::PlaceMarker,
    parts::ImportParts,
    assign::AssignPart,
    // A part whose type is not known yet
    placeholder::AssignPlaceholder,
    library_update::UpdateLibraryCopies,
    macro_insert::InsertMacro,
    join::JoinDevice,
    part_edit::EditPartCopy,
    part_delete::DeletePartCopy,
    // A frame or title block changed on the page
    form_edit::EditForm,
    // The columns and rows of a frame
    form_edit::SetFrameGrid,
    // A frame of a page's own, a title block of a frame's own
    form_edit::MakeFormOwn,
    // The frames of a template taken
    frames_adopt::AdoptFrames,
    workshop::SetWorkshopStock,
    // The fields of the project's own
    user_prop::DefineUserProp,
    user_prop::RenameUserProp,
    user_prop::RemoveUserProp,
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
    layout::MoveLayoutLabel,
    layout::ResetLayoutLabels,
    layout::PlaceAssembly,
    layout::AutoFill,
    layout::CreateEnclosureFromPart,
    layout::EditLayoutSpace,
    layout::EditEnclosure,
    layout::EditSurface,
}
