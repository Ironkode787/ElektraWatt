// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! All domain queries and the tracked-read context `Q` (ARCH §6, ADR-0002, ADR-0003).
//!
//! # Shape
//!
//! A derived value is a pure function over a tracked-read context (ADR-0002 decision 1):
//!
//! ```text
//! trait Query { type Key; type Value; const FAMILY; const POLICY; fn run(q: &Q, key: &Key) -> Arc<Value>; }
//! q.get::<DeviceTag>(dev)            // cached per Q, cycle-checked, reported to the tracker
//! q.try_get::<StructureOf>(parent)   // the same, but a cycle is an Err(Cycle), never a panic
//! q.devices().get(id)                // a tracked row read
//! q.idx().reps_by_page(page)         // a tracked index-bucket read
//! ```
//!
//! Phase 1 is **cold** (ADR-0002 decision 2): every family is computed on demand and cached only for the
//! lifetime of one [`Q`]. Each family declares its initial [`Policy`] (ARCH §6.4) for the memo engine of
//! the next phase, and every read is reported (to the memo store, and to a [`RecordingTracker`] when one is given), so that
//! `ew-memo` can record dependencies without any change to query code.
//!
//! # How "an untracked read does not compile" is kept (ARCH §3 rule 1, ADR-0002 amendment 2)
//!
//! - `ew-model`'s raw read API exists only with its `raw-read` feature, which this crate, `ew-commands`
//!   and `ew-store` alone enable on normal edges (the feature rule of `cargo xtask rules`).
//! - A query receives `&Q` and nothing else. `Q` owns its snapshot privately; no method or field of `Q`,
//!   [`TableView`] or [`IdxView`] hands out the `Model`, a raw `Table`, a raw index or an `Op`, so code
//!   holding only `&Q` cannot read untracked (the `compile_fail` example on [`Q`] and
//!   `tests/api.rs` guard it).
//! - Untracked state stays out of reach as well: `Q`'s `Debug` prints no revision, and the evaluated keys are read
//!   through [`Q::diagnostics`], which takes `&mut Q` (a second `compile_fail` example guards it). A
//!   [`DeriveNote`] is part of the value that holds it, where a rule reads it.
//! - The public surface of this crate carries no `Model` except as the input of [`Q::new`].
//!
//! # Families
//!
//! A family is a type that implements [`Query`]; its `FAMILY` name labels it in statistics, notes and
//! `ew verify-derived`, and its `POLICY` says whether a memo store keeps it. A `Q` whose request cache meets
//! two query types with one family name panics (the cache keys families by name).
//!
//! # Determinism
//!
//! No hash map feeds a result, no float is used, and orders are total (IDs break ties), so two `Q`s over
//! equal models give equal values (NFR-REL-07, FR-NUM-028).

#![forbid(unsafe_code)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

// `TEXTS`, `KEYS`, `SOURCES`, `text!` and `key!`, before the modules that name a key.
ew_l10n::texts!(crate);

mod accessories;
mod assembly;
mod bom;
mod bond_carrier;
pub mod box_edges;
mod bus;
mod bus_symbol;
mod cell;
mod channel_index;
mod component;
mod consumables;
mod cp_point;
mod cps;
mod cut_list;
mod drawn_path;
mod flow;
mod flow_distance;
mod fluid;
mod fluid_media;
mod fluid_rows;
mod index;
mod ip;
mod items;
mod joins;
// The layout's pure geometry, the compatibility table and the pose validator
mod labels;
pub mod layout;
mod links;
mod marking;
mod memo_keys;
mod memo_store;
mod names;
mod nets;
mod note;
mod numbering;
mod own_values;
mod pages;
pub mod part_fields;
mod part_fit;
mod part_text;
mod pattern;
mod piece_symbol;
mod plc;
mod potentials;
mod profile;
mod q;
mod query;
mod rework;
mod settings;
mod stitch;
mod strip_assembly;
mod strip_bridges;
mod structure;
mod tag;
pub mod text;
mod text_frame;
mod title_block;
mod toc;
mod tracker;
mod treatment;
mod valve_cells;
mod wire_profile;
mod wire_props;
mod wire_record;
mod wires;
mod xref;
mod xref_format;

pub use accessories::{
    BracketFrom, DeclaredBracket, DerivedAccessory, PartAccessories, StripBracket, accessory_copy,
    accessory_part, carrier_accessories, placed_brackets, strip_accessories, strip_bracket,
};
pub use assembly::{
    AssemblyBreakMarks, BreakSide, assembly_members, device_descendants, order_code_part, order_code_text,
    overview_rep, overview_start_page,
};
pub use bom::{BomMode, BomRow, BomRows, DevicePartLines, LineSource, PartLine};
pub use cell::{
    Cell, CellOf, FrameGridOf, FrameSettings, FrameSettingsValue, RepAnchorCell, anchor_point,
    anchor_point_of, cell_in_grid, column_label, default_frame_grid, frame_form_from_settings, frame_form_of,
    frame_grid, in_drawing_area, page_frame_form, prints_cells, row_label, starter_form,
};
pub use channel_index::{ChannelsByCp, ChannelsByModule, ModuleChannelIds};
pub use consumables::{Consumables, EndForm, EndUse, MarkerUse, WireUse, consumable_rows};
pub use cp_point::{
    cp_cell, cp_end, cp_point, rep_cp_end, rep_drawn_cps, segment_point, strip_segment_of,
    strip_terminal_end, terminal_drawn_function,
};
pub use cps::{ClampData, Cp, FunctionCps, terminal_levels};
pub use cut_list::{CUT_INCREMENT, CutFor, CutGroup, CutRow, CutWhat, cut_rows, cut_to};
pub use drawn_path::{
    dir_vertical, drawn_path, on_segment, oriented_dir, page_grid, read_connection, read_junction,
    stored_path,
};
pub use part_fit::{Misfit, fit_report, part_fit};
pub use piece_symbol::{piece_symbol_copy, placed_symbol};
// The derived-index families of the connectivity (ARCH §6.3 rule 5) and the interruption-point
// pairing split out of the stitch graph.
pub use index::{
    BondIndex, BondVias, DefPoints, Index, IpFunctions, Link, PageIds, PagelessLinks, RailTerminals,
};
pub use ip::{IpInfoOf, IpPairing, IpPairingQ};
// Page items and structure boxes, derived.
pub use items::{
    BINDABLE, CableMark, IpStructure, RepStructureBox, SYMBOL_BINDABLE, bound_text, box_caption, cable_mark,
    cable_size_text, connection_marker, connection_number, effective_marker_text, free_text, group_marker,
    grouped_connections, live_connections, marker_mode, outline_contains, shows_marker, strips_named_on,
};
pub use joins::{JoinWiring, cp_label, cp_pin, cp_tag_from, wire_join};
pub use links::{
    BondPathsQ, DeclaredLink, FunctionLinksOf, RailBondsQ, RailLocation, RailTerminalOf, device_location,
};
pub use memo_keys::{MemoInput, PartKey};
pub use memo_store::MemoStore;
// The carrier of a 0 V–PE bond (EW-POT-001's quick fix).
pub use bond_carrier::{BondCarrierQ, Carrier};
// The bus model's derived data.
pub use bus::{
    BusSegment, BusSegments, NetMaster, NetMembers, NetMembersOf, SegmentMember, bus_address_text,
    bus_core_ends, bus_core_pins, bus_end_label, bus_termination_text, bus_word, cable_bus_line,
    external_partner_text, net_address_text, potential_member_count, station_bus_address,
};
pub use bus_symbol::{
    BusInterface, DrawnRows, DrawnSymbol, bus_interfaces, bus_part, bus_pins, bus_symbol, carried_symbol,
    compartment_address, drawn, has_bus_ports, interface_words, is_bus_copy, is_bus_symbol, own_bus_symbol,
    part_bus_symbol, socket_mismatch, texts_clear,
};
pub use flow::{Flow, FlowBfs, Load, PotentialFlow, Upstream};
pub use flow_distance::{CpFlowDistanceOf, CpFlowDistances};
pub use fluid::{
    FluidNets, FluidTube, FluidTubes, Galleries, ValvePort, ValveSlot, ValveSlots, is_fitting, is_fluid,
    is_valve_terminal, open_working_ports, parent_chain, positions_of, reached_device, tracked_process_tag,
    tracked_target, valve_functions, valve_positions,
};
pub use fluid_media::{Effective, MediumKey, MediumOrigin, MediumZones, TubePropsOf, TubeSource, node_net};
pub use fluid_rows::{
    MediumRows, TubeEnd, TubeRow, TubeRows, ValveTerminalRows, VtSlot, is_blanking_plate, langs,
    line_kind_text, line_tag, location_text, medium_text, part_number, port_role_text, tube_spec_text,
    valve_function,
};
pub use names::{
    AttrConflict, DefAttr, PotentialAttrsOf, PotentialAttrsV, PotentialNameOf, PotentialNames, cp_text,
    potential_named, ranked_defs,
};
pub use nets::{CpRef, NetId, PageIpNets, PageLocalNets, ZeroVoltIps};
pub use note::{DeriveNote, NoteFact, SchemeIssue};
pub use numbering::{WireNumber, WireNumbers, orient_wire};
pub use pages::{
    GeneratedPage, Origin, PageEntry, PageKey, PageOrder, PageOrderV, PageRef, PageRefStructure, PageTitle,
    SectionStructure, drawn_page_key, folio_label, map_ml, print_language, section_ranks, text_in,
};
pub use part_text::{
    main_part, manufacturer_copy, manufacturer_name, part_confidence, printed_number, printed_part,
    tech_data_text, tech_value,
};
pub use pattern::{PatternError, expand_pattern};
pub use plc::{
    AddressIndex, AddressUse, ChannelAddr, ChannelAddresses, ChannelStates, ChannelTargets, ChannelTexts,
    ChannelsByTarget, EffectiveState, ModuleOrder, ModuleStart, ModuleStartOf, ModuleStarts, PlcAddressText,
    PlcAddressTexts, PlcRow, PlcRows, Slot, SolenoidPositions, SolenoidSlot, Start, address_profile,
    address_profile_named, auto_address, builtin_profiles, canonical, channel_data_type, channel_kind_word,
    channel_pin, coil_cp, coil_host, default_profile, driven_device, iec_ident, iolink_device, is_iec_ident,
    module_io, module_part, not_a_module, parse_plc_address, parse_plc_start, pin_pattern_error,
    pinned_profile, port_windows, process_tag_parts, process_tag_scheme, profile_of, profiles_at_hand,
    solenoid_channel, solenoid_coil, start_example, station_of, station_tree, stations, stored,
    symbol_template_error, unreadable_profile, valve_solenoids,
};
pub use potentials::{
    Attr, AttrSource, BondLink, Component, NameSource, Partition, Potential, PotentialId, PotentialPartition,
    Signal, bonds_of_potential, bus_line_cables, bus_line_pages, potential_id_at, potential_part,
};
pub use profile::{
    CounterScope, NumberingScheme, NumberingSettingsOf, ProfileStatus, XrefFormat,
    by_version as profile_by_version, numbering_settings, scheme_for, shipped as shipped_profiles,
};
pub use q::{Cycle, IdxView, Q, TableView};
pub use query::{Changes, Policy, Query};
// The rework list between a release and now (doc 13 §3.5).
pub use rework::{
    FollowedFrom, ReworkAction, ReworkChange, ReworkItem, ReworkReason, ReworkSubject, WirePair,
    WorkshopSetting, rework, wire_changes,
};
pub use settings::{
    DrawingSettingsQ, Member, SettingsIssue, SettingsMember, TextStyles,
    check_member as check_settings_member, company_codes, text_styles,
};
pub use stitch::{EdgeKind, IpInfo, Level, PairMode, ShortSource, StitchGraph, StitchGraphQ};
pub use strip_assembly::{AssemblyItem, AssemblyItemKind, OnRail, StripAssembly, assembly_rows};
pub use strip_bridges::{BridgeGroup, StripBridges, bridge_groups};
pub use structure::{
    DevicesByStructure, PageDisplayStructure, PageStructure, ResolvedSlot, ResolvedStructure, SlotSource,
    StructureOf, StructurePath, would_print_location,
};
pub use tag::{
    DeviceTag, DisplayedTag, MainFunction, MainRep, MainRepRows, Tag, TagSource, cable_core_page,
    level_short_tag, main_rep_with, numbering_format, rep_level, rule_product, short_tag,
};
// How a value of the domain is printed.
pub use labels::{
    GENERIC_PROFILE, LabelColumn, LabelRecord, LabelRows, LabelSet, LabelTarget, label_profile,
};
pub use marking::{MarkedBy, MarkerText, NotMarked, marker_end_text, shorten, wire_marker_part};
pub use text::{
    awg_gauge, bus_address, channel_address, channel_address_words, colour_code_text, colour_text, csa_text,
    csa_with_unit, mm_exact, mm_hundredth, mm_micro, mm_padded, mm_tenth, mm_tenth_float, mm_tenth_short,
    mm_truncated, potential_type_code, slot_short, slot_text,
};
pub use text_frame::{TextFrame, slot_shown, tag_slot_shown, terminal_symbol, text_frame};
pub use title_block::{
    TbValue, TitleBlock, TitleBlockValues, current_revision, last_changed_in, last_released_revision,
    open_revision, placeholder_paths, recorded_changed_pages, revision_label_key,
};
pub use toc::{TocRow, TocRows};
pub use tracker::{DepKey, Recorded, RecordingTracker};
pub use treatment::{
    Assumed, ClampMatch, ConductorMatch, EndTreatment, RuleGives, TREATMENT_RULES, TreatedBy, TreatmentRule,
    Undecided, clamp_conductors, end_connection, end_through, rule_for, stock_part,
};
pub use valve_cells::{
    CELL_STEPS, GALLERY_PORTS, PlugPin, SlotFrame, box_frame, device_fits_terminal, fits_terminal,
    graphics_x_extent, is_fluid_terminal, is_multipin_terminal, is_supply_def, plug_pins, solenoid_pin,
    terminal_cells, terminal_supply, wired_solenoid_channel,
};
pub use wire_profile::{
    Attrs, ColourProfile, ProfileKey, ProfileRowFor, SizeProfile, WireDefaultProps, WireProfilesQ,
    csa_short_of, current_table_basis,
};
pub use wire_props::{
    ConnWireProps, EffectiveProps, EndClass, EndRating, EndScope, PropSource, Prov, SizeRule, Unrated,
    WirePropsMap, WirePropsOf, pick, unfed_protection, unrated, wire_part_of, wire_props_below_overrides,
    wire_rating,
};
pub use wire_record::{EndRecord, WireLength, WireRecord, WireRecordFlags, WireRecordSet, wire_length};
pub use wires::{EndKind, PhysicalWire, PhysicalWiresOf, WireEnd};
// A wire's own values (doc 13 §2.3).
pub use own_values::{HandEnd, WireValues, hand_ends, members, wire_values, with_members};

// The report-pages provider hook of `page_order` and the drawn-page order.
pub use pages::{DrawnPageOrder, ReportPagesProvider};

// The constituent behind each level-7 wire value.

// Cross-references (relations, formats, texts) and the format token engine.
pub use xref::{TargetRole, XrefKind, XrefRelationOf, XrefSettingsOf, XrefTarget, XrefText, XrefTextOf};
pub use xref_format::{ItemOptions, ItemValues, Rendered, XrefToken, parse_format, render_item, render_list};
