// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The hand arms: generators of well-formed steps and deep states, each building a step from the live records.
//! The schema-driven default arm ([`super::schema_arm`]) fuzzes every registered command beside them, so a new
//! command needs no hand arm; [`ARMS`] names registered commands only (`every_hand_arm_names_a_registered_command`).
//! An arm returns `None` when the snapshot has nothing to target (the no-op rule of doc 18 §3.3).
//!
//! The command decides whether a step is accepted. Every arm mixes, by a dice roll:
//! - steps built from the live records, which a command should mostly commit, and steps a command's own rules
//!   refuse (an off-grid position, a duplicate key, a cycle): both `Verdict::Either`;
//! - steps invalid in a way no command rule decides, `Verdict::Refuse`: an ID that names no record or a record of
//!   another kind, parameters that fail the schema check or parse: the model must refuse them and change nothing;
//! - after-commit checks: counts of the records the command creates, the primary record, and field values.

use serde_json::{Value, json};

use std::sync::OnceLock;

use super::live::{Live, PageInfo, PartTemplate, StripInfo, SymInfo, id_json, id_text};
use super::rng::SplitMix64;
use super::schema_arm;
use super::step::{Step, Table, typed};

mod devices;
mod fields;
mod layout;
mod library;
mod pages;
mod plc;
mod project;
mod revision;
mod sheet;
mod wiring;

use devices::*;
use fields::*;
pub use layout::*;
pub use library::*;
use pages::*;
use plc::*;
use project::*;
pub use revision::*;
use sheet::*;
use wiring::*;

/// The unit of a move: three steps of the page grid in nm (7 937 500 nm, a multiple of the grid of the golden
/// pages).
pub const MOVE_UNIT: i64 = 7_937_500;

/// The string values a command's schema allows at a path of member names, read from the registry: the arms
/// never copy a list the registry holds ([`schema_arm::strings_at`]).
fn registry_strings(command: &str, path: &[&str]) -> Vec<&'static str> {
    schema_arm::strings_at(ew_commands::CommandRegistry::builtin(), command, path)
}

/// Commands that no submit path of the session commits, so that their arms only ever refuse: `revision.approve` is
/// recorded only by `Session::approve` on a saved generation, `revision.release` only by
/// `Session::release` after the archive and the tag.
pub const SESSION_REFUSES: &[&str] = &["revision.approve", "revision.release"];

/// One generator arm.
pub struct Arm {
    /// The canonical command name.
    pub command: &'static str,
    /// The relative frequency among the steps.
    pub weight: usize,
    /// Builds a step, or `None` when the snapshot has no target.
    pub build: fn(&Live, &mut SplitMix64) -> Option<Step>,
}

/// Every arm, in command-name order. The coverage test compares the names with the registry.
pub const ARMS: &[Arm] = &[
    Arm {
        command: "assembly.attach",
        weight: 3,
        build: assembly_attach,
    },
    Arm {
        command: "bond.create",
        weight: 2,
        build: bond_create,
    },
    Arm {
        command: "box.black",
        weight: 2,
        build: box_black,
    },
    Arm {
        command: "box.reshape",
        weight: 3,
        build: box_reshape,
    },
    Arm {
        command: "box.structure",
        weight: 2,
        build: box_structure,
    },
    Arm {
        command: "cable.create",
        weight: 2,
        build: cable_create,
    },
    Arm {
        command: "check.unwaive",
        weight: 1,
        build: check_unwaive,
    },
    Arm {
        command: "check.waive",
        weight: 1,
        build: check_waive,
    },
    Arm {
        command: "conductor.add",
        weight: 2,
        build: conductor_add,
    },
    Arm {
        command: "connection.delete",
        weight: 3,
        build: connection_delete,
    },
    Arm {
        command: "connection.draw",
        weight: 4,
        build: connection_draw,
    },
    // Cycle 33
    Arm {
        command: "connection.draw_many",
        weight: 2,
        build: connection_draw_many,
    },
    Arm {
        command: "connection.draw_to_ip",
        weight: 3,
        build: connection_draw_to_ip,
    },
    Arm {
        command: "connection.route",
        weight: 3,
        build: connection_route,
    },
    Arm {
        command: "device.create",
        weight: 3,
        build: device_create,
    },
    Arm {
        command: "device.link",
        weight: 2,
        build: device_link,
    },
    Arm {
        command: "device.place",
        weight: 6,
        build: device_place,
    },
    Arm {
        command: "edit.delete",
        weight: 3,
        build: edit_delete,
    },
    Arm {
        command: "edit.paste",
        weight: 3,
        build: edit_paste,
    },
    Arm {
        command: "external_ref.create",
        weight: 2,
        build: external_ref_create,
    },
    Arm {
        command: "fields.set",
        weight: 8,
        build: fields_set,
    },
    Arm {
        command: "fields.set_many",
        weight: 3,
        build: fields_set_many,
    },
    Arm {
        command: "ip.arrange",
        weight: 3,
        build: ip_arrange,
    },
    Arm {
        command: "ip.continue",
        weight: 2,
        build: ip_continue,
    },
    Arm {
        command: "ip.place",
        weight: 3,
        build: ip_place,
    },
    Arm {
        command: "item.place",
        weight: 3,
        build: item_place,
    },
    Arm {
        command: "junction.edit",
        weight: 3,
        build: junction_edit,
    },
    Arm {
        command: "library.embed",
        weight: 2,
        build: library_embed,
    },
    Arm {
        command: "library.update",
        weight: 2,
        build: library_update,
    },
    Arm {
        command: "macro.insert",
        weight: 2,
        build: macro_insert,
    },
    Arm {
        command: "marker.place",
        weight: 3,
        build: marker_place,
    },
    Arm {
        command: "numbering.renumber",
        weight: 2,
        build: numbering_renumber,
    },
    Arm {
        command: "page.create",
        weight: 3,
        build: page_create,
    },
    Arm {
        command: "page.delete",
        weight: 1,
        build: page_delete,
    },
    Arm {
        command: "page.renumber",
        weight: 2,
        build: page_renumber,
    },
    Arm {
        command: "page.reorder",
        weight: 2,
        build: page_reorder,
    },
    Arm {
        command: "part.assign",
        weight: 2,
        build: part_assign,
    },
    Arm {
        command: "parts.import",
        weight: 2,
        build: parts_import,
    },
    Arm {
        command: "plc.bind_channel",
        weight: 3,
        build: plc_bind_channel,
    },
    Arm {
        command: "plc.config_project.create",
        weight: 2,
        build: plc_config_project_create,
    },
    Arm {
        command: "plc.set_module_part",
        weight: 1,
        build: plc_set_module_part,
    },
    Arm {
        command: "plc.set_profile",
        weight: 1,
        build: plc_set_profile,
    },
    Arm {
        command: "plc.set_start",
        weight: 2,
        build: plc_set_start,
    },
    Arm {
        command: "potential.define",
        weight: 3,
        build: potential_define,
    },
    Arm {
        command: "project.assets.purge",
        weight: 1,
        build: project_assets_purge,
    },
    Arm {
        command: "project.checks.set",
        weight: 1,
        build: project_checks_set,
    },
    Arm {
        command: "project.company.set",
        weight: 1,
        build: project_company_set,
    },
    Arm {
        command: "project.drawing.set",
        weight: 1,
        build: project_drawing_set,
    },
    Arm {
        command: "project.frames.set",
        weight: 1,
        build: project_frames_set,
    },
    Arm {
        command: "project.languages.set",
        weight: 1,
        build: project_languages_set,
    },
    Arm {
        command: "project.settings.set",
        weight: 1,
        build: project_settings_set,
    },
    Arm {
        command: "project.supply.set",
        weight: 2,
        build: project_supply_set,
    },
    Arm {
        command: "rep.exchange_symbol",
        weight: 3,
        build: rep_exchange_symbol,
    },
    Arm {
        command: "rep.join_device",
        weight: 2,
        build: rep_join_device,
    },
    Arm {
        command: "rep.move",
        weight: 5,
        build: rep_move,
    },
    Arm {
        command: "rep.set_anchor_policy",
        weight: 2,
        build: rep_set_anchor_policy,
    },
    Arm {
        command: "revision.approve",
        weight: 1,
        build: revision_approve,
    },
    Arm {
        command: "revision.create",
        weight: 3,
        build: revision_create,
    },
    Arm {
        command: "revision.release",
        weight: 1,
        build: revision_release,
    },
    Arm {
        command: "revision.set_author",
        weight: 1,
        build: revision_set_author,
    },
    Arm {
        command: "revision.withdraw",
        weight: 1,
        build: revision_withdraw,
    },
    Arm {
        command: "rep.transform",
        weight: 4,
        build: rep_transform,
    },
    Arm {
        command: "report_block.create",
        weight: 2,
        build: report_block_create,
    },
    Arm {
        command: "section.create",
        weight: 2,
        build: section_create,
    },
    Arm {
        command: "structure.create_node",
        weight: 2,
        build: structure_create_node,
    },
    Arm {
        command: "terminal.add",
        weight: 3,
        build: terminal_add,
    },
    Arm {
        command: "terminal.designate",
        weight: 3,
        build: terminal_designate,
    },
    Arm {
        command: "terminal.insert",
        weight: 3,
        build: terminal_insert,
    },
    Arm {
        command: "terminal.move",
        weight: 3,
        build: terminal_move,
    },
    Arm {
        command: "valve.add",
        weight: 3,
        build: valve_add,
    },
    // Last, so the per-arm seeds of the arms before them stay as they were.
    Arm {
        command: "bus.logical_net.create",
        weight: 1,
        build: bus_logical_net_create,
    },
    Arm {
        command: "bus.physical_net.create",
        weight: 1,
        build: bus_physical_net_create,
    },
    // Last, for the same reason.
    Arm {
        command: "part.edit_copy",
        weight: 2,
        build: part_edit_copy,
    },
    // Last, for the same reason.
    Arm {
        command: "layout.space.create",
        weight: 2,
        build: layout_space_create,
    },
    Arm {
        command: "layout.space.delete",
        weight: 1,
        build: layout_space_delete,
    },
    Arm {
        command: "layout.enclosure.create",
        weight: 2,
        build: layout_enclosure_create,
    },
    Arm {
        command: "layout.enclosure.delete",
        weight: 1,
        build: layout_enclosure_delete,
    },
    Arm {
        command: "layout.carrier.add",
        weight: 3,
        build: layout_carrier_add,
    },
    Arm {
        command: "layout.carrier.edit",
        weight: 2,
        build: layout_carrier_edit,
    },
    Arm {
        command: "layout.carrier.delete",
        weight: 1,
        build: layout_carrier_delete,
    },
    Arm {
        command: "layout.place",
        weight: 4,
        build: layout_place,
    },
    Arm {
        command: "layout.move",
        weight: 2,
        build: layout_move,
    },
    Arm {
        command: "layout.remove",
        weight: 1,
        build: layout_remove,
    },
    Arm {
        command: "layout.lock",
        weight: 1,
        build: layout_lock,
    },
    // Appended last, as the layout arms.
    Arm {
        command: "page.surface_view.set",
        weight: 2,
        build: page_surface_view_set,
    },
    // FT-840: appended last, for the same reason.
    Arm {
        command: "layout.place_assembly",
        weight: 2,
        build: layout_place_assembly,
    },
    Arm {
        command: "layout.auto_fill",
        weight: 2,
        build: layout_auto_fill,
    },
    Arm {
        command: "layout.enclosure.create_from_part",
        weight: 1,
        build: layout_enclosure_create_from_part,
    },
    Arm {
        command: "layout.space.edit",
        weight: 1,
        build: layout_space_edit,
    },
    Arm {
        command: "layout.enclosure.edit",
        weight: 1,
        build: layout_enclosure_edit,
    },
    Arm {
        command: "layout.surface.edit",
        weight: 1,
        build: layout_surface_edit,
    },
    // Last, so that the arms before it keep their seeds in `every_arm_commits_on_golden_a`
    Arm {
        command: "connection.route_many",
        weight: 2,
        build: connection_route_many,
    },
];

/// The representation types a page type allows (`allowed_rep_types` of `ew-commands`, in JSON form).
fn allowed(kind: &str) -> &'static [&'static str] {
    match kind {
        "multi_line" => &["multi_line", "contact_image", "strip_segment", "graphic"],
        "single_line" => &["single_line"],
        "overview" | "assembly_overview" => &["overview"],
        "bus_topology" => &["topology", "graphic"],
        "fluid" => &["fluid", "multi_line"],
        "graphic" => &["graphic"],
        _ => &[],
    }
}

fn pick2<'a, T>(items: &'a [T], rng: &mut SplitMix64) -> Option<(&'a T, &'a T)> {
    if items.len() < 2 {
        return None;
    }
    let a = rng.below(items.len());
    let mut b = rng.below(items.len() - 1);
    if b >= a {
        b += 1;
    }
    Some((&items[a], &items[b]))
}

fn cp_end(function: &Value, idx: u16) -> Value {
    json!({"cp": {"function": function, "idx": idx}})
}

fn small_idx(rng: &mut SplitMix64) -> u16 {
    u16::try_from(rng.below(4)).unwrap_or(0)
}

/// A letter of the identifier sets of the golden project.
fn letter(rng: &mut SplitMix64) -> &'static str {
    ["K", "Q", "F", "S", "X", "A", "M"][rng.below(7)]
}

/// The builder of an arm's setup steps ([`setup_of`]).
pub type Setup = fn(&Live, &mut SplitMix64) -> Option<Vec<Step>>;

/// The setup steps of an arm whose command finds nothing to act on in Golden A, for
/// `every_arm_commits_on_golden_a` (the fuzz loop reaches such states by itself): committed in order
/// before the arm's step is built and undone after it. `None` for every other command.
pub fn setup_of(command: &str) -> Option<Setup> {
    match command {
        // Golden A has no D3-eligible contact and no device that takes one: a contact placed as a new device is the
        // source, a device created with an unplaced function of the contact's FnDef the slot it fills.
        "rep.join_device" => Some(join_setup),
        _ => None,
    }
}

/// One step of a setup chain ([`setup_chain_of`]), built from the live records once the previous step committed.
pub type ChainStep = fn(&Live, &mut SplitMix64) -> Option<Step>;

/// The setup chain of an arm whose command needs records that Golden A lacks and that earlier setup steps create (so
/// each step is built only after the previous one committed), for `every_arm_commits_on_golden_a`:
/// committed in order before the arm's step is built and undone after it. A step that finds its record already there
/// returns `None` and is skipped. `None` for every other command.
pub fn setup_chain_of(command: &str) -> Option<&'static [ChainStep]> {
    const SPACE: &[ChainStep] = &[setup_space];
    const ENCLOSURE_PART: &[ChainStep] = &[setup_space, setup_enclosure_part];
    const ENCLOSURE: &[ChainStep] = &[setup_space, setup_enclosure_part, setup_enclosure];
    const RAIL: &[ChainStep] = &[setup_space, setup_enclosure_part, setup_enclosure, setup_rail];
    const PLACED: &[ChainStep] = &[
        setup_space,
        setup_enclosure_part,
        setup_enclosure,
        setup_rail,
        setup_place,
    ];
    const UNPLACED: &[ChainStep] = &[setup_unplace_member];
    match command {
        "layout.space.delete" => Some(SPACE),
        "layout.enclosure.create" => Some(ENCLOSURE_PART),
        "layout.enclosure.delete" | "layout.carrier.add" => Some(ENCLOSURE),
        "layout.carrier.edit" | "layout.carrier.delete" | "layout.place" => Some(RAIL),
        "layout.move" | "layout.remove" | "layout.lock" => Some(PLACED),
        // Golden A's assemblies are placed; one member is taken off its rail first.
        "layout.place_assembly" | "layout.auto_fill" => Some(UNPLACED),
        _ => None,
    }
}
