// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The v0 editor commands (doc 06 §3.1, within the v0 record set). Every command struct is its own
//! parameter struct, with serde, a JSON Schema and `deny_unknown_fields`.
//!
//! The commands are listed once, in the `commands!` invocation below; [`CommandRegistry::builtin`] holds every one by
//! name. The book's command table (`docs/book/src/reference/commands/index.md`) is generated from their schemas.
//!
//! Every command is of stability `Preview` and undoable except `project.assets.purge`, a marker line whose file side
//! (deleting asset files nothing names) runs in the session (ADR-0008 amendment 1, FT-380), and `revision.release`,
//! the release's stamp, which a released revision's immutability makes a barrier (FR-REV-009); every one is of exposure
//! class `Edit` except `project.checks.set`, `check.waive` and `check.unwaive`, which are `HumanOnly` (ADR-0008 amendment
//! 3: they decide a check outcome), `revision.set_author`, `revision.withdraw`, `revision.approve` and
//! `revision.release`, which are `HumanOnly` because they decide the four-eyes sign-off and the release history
//! (WP-S1.18 D4, D5, D9, D13),
//! and `project.assets.purge`, which is `HumanOnly` because deleted files do not come back (owner ruling, 2026-10-23).
//! `external.apply` is of exposure class `Internal` and stability `Internal`: only the session's external-change apply
//! submits it, from the origin `external` (WP-S1.17 D22, decision log ID-1284).

mod assembly;
mod assets;
mod assign;
mod box_reshape;
// WP-S1.21 part 4 (D11)
mod bus;
mod checks;
pub mod common;
mod company;
mod connection;
mod device;
mod drawing;
mod edit;
mod exchange;
// WP-S1.17 part 6 (D22)
mod external;
mod fields;
mod frames;
pub(crate) mod items;
mod join;
mod languages;
// WP-S4.01 part 1
mod layout;
pub(crate) mod library;
mod library_update;
// WP-S1.19 part 5
mod macro_insert;
// WP-S1.17 part 5
mod merge;
mod page;
// WP-S2.07 part 4a
mod part_edit;
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

pub use assembly::ChildSpec;
pub use assign::{ASSIGN_BULK_FROM, AssignModeParam};
pub use checks::{CHECKS_MEMBER, SHIPPED_CHECK_SCHEMES, WAIVER_REASON_MIN_CHARS};
pub use common::{EndpointParam, KindName, TargetRef};
pub use company::COMPANY_MEMBER;
pub use connection::{IpAtEnd, WiringChoice};
pub use device::{ConnectHint, PlaceTarget, allowed_rep_types};
pub use drawing::{Crossings, DRAWING_MEMBER, MarkerModeParam};
pub use edit::BULK_ABOVE;
pub use exchange::{ExchangePin, PinPairs, derive_pin_map};
pub use fields::{ASPECTS, SET_MANY_MAX_EDITS, SETTABLE};
pub use frames::{FRAMES_MEMBER, FrameForPageType};
pub use items::{
    AlignParam, DashParam, FreeTextParam, GraphicParam, ImageParam, ItemKindParam, ShapeParam, StrokeParam,
};
pub use layout::{
    CarrierKindParam, CarrierParamsParam, CarrierThen, FillOrder, LAYOUT_BULK_ABOVE, PlaceMode, PlaceOnParam,
};
pub use library::library_copy_hash;
pub use library_update::{ChangeClass, CopyDiff, CpMapParam, FieldChange, classify_update};
pub use macro_insert::{
    InstantiateError, Instantiated, MACRO_FIELDS, MACRO_KINDS, binding_field_ok, coerce_value, instantiate,
    pick_variant,
};
#[cfg(feature = "testkit")]
pub use merge::record_file_of;
pub use merge::{ConflictChoice, Take};
pub use page::{LabelModeParam, PageScope};
pub use part_edit::PART_EDITABLE;
pub use parts::import_counts;
pub use paste::{Fragment, FragmentNode, PASTE_KINDS, PASTE_MAX_RECORDS};
pub use plc::{StartDir, is_on_module};
pub use renumber::{DeviceScope, NumberingScheme, RenumberOptions};
pub use revision::{ApprovedPack, LABEL_ORDER_RULE, NewRevisionState, ReleasedArchive, RevisionPageNote};
pub use settings::{SETTABLE_SETTINGS, SettableSetting};
pub use structure_box::BoxStyleParam;
pub use supply::SETTABLE_SUPPLY;
pub use terminal::{ConductorSpec, TerminalDesignation, TerminalSpec};

use crate::registry::CommandRegistry;

// map: commands: a new command goes in commands! as module::Type; its title is the key command-<name, . as ->.
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
    // WP-S4.01 part 3 (D9)
    page::SetSurfaceView,
    structure::CreateStructureNode,
    structure::CreateSection,
    structure::CreateReportBlock,
    structure::CreateExternalRef,
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
    // WP-S2.04 part 3
    connection::DrawToIp,
    connection::ArrangeIps,
    // Cycle 33: the chain of bridges of Edit ▸ Bridge terminals
    connection::ConnectMany,
    terminal::AddTerminals,
    terminal::DesignateTerminals,
    terminal::CreateCable,
    terminal::AddConductors,
    // WP-S2.07 part 1: the strip editor's insert and move
    terminal::InsertTerminals,
    terminal::MoveTerminals,
    fields::SetFields,
    // WP-S2.05 part 2
    fields::SetManyFields,
    edit::DeleteSelection,
    renumber::RenumberDevices,
    settings::SetProjectSettings,
    assembly::AttachChildren,
    assembly::AddValve,
    assembly::LinkDevices,
    plc::CreatePlcConfigProject,
    // WP-S1.21 part 4 (D11)
    plc::BindChannel,
    plc::SetModulePart,
    // WP-S1.21 part 3 (D8)
    plc::SetStart,
    plc::SetProfile,
    // WP-S1.21 part 4 (D11)
    bus::CreateLogicalNet,
    bus::CreatePhysicalNet,
    supply::SetProjectSupply,
    library::EmbedLibraryCopies,
    revision::CreateRevision,
    revision::SetRevisionAuthor,
    revision::WithdrawRevision,
    // WP-S1.18 part 3: the sign-off (D9)
    revision::ApproveRevision,
    // WP-S1.18 part 4: the release's stamp (D13)
    revision::ReleaseRevision,
    company::SetCompany,
    frames::SetFrameDefaults,
    languages::SetProjectLanguages,
    checks::SetCheckSchemes,
    // WP-S1.16 part 5: waiver recording (D7, FT-494)
    checks::WaiveFinding,
    checks::UnwaiveFinding,
    // WP-S1.17 part 5: conflict records (D19)
    merge::ResolveConflicts,
    // WP-S1.17 part 6: an external change (D22)
    external::ApplyExternal,
    assets::PurgeAssets,
    exchange::ExchangeSymbol,
    drawing::SetDrawing,
    // WP-S2.05 part 3
    paste::PasteFragment,
    // WP-S2.04 part 3
    box_reshape::ReshapeBox,
    // WP-S2.04 part 4a
    structure_box::CreateStructureBox,
    items::PlaceItem,
    items::PlaceMarker,
    // FT-410
    parts::ImportParts,
    // WP-S1.19 part 3
    assign::AssignPart,
    library_update::UpdateLibraryCopies,
    // WP-S1.19 part 5
    macro_insert::InsertMacro,
    // WP-S2.18 D3 (FT-628)
    join::JoinDevice,
    // WP-S2.07 part 4a (D20, FT-394)
    part_edit::EditPartCopy,
    // WP-S4.01 part 1 (D6)
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
    // WP-S4.02 part 2 (D6, D7, FT-840)
    layout::PlaceAssembly,
    layout::AutoFill,
    layout::CreateEnclosureFromPart,
    layout::EditLayoutSpace,
    layout::EditEnclosure,
    layout::EditSurface,
}
