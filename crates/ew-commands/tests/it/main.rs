// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-commands`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod black_box_pins;
mod cmd_assembly;
mod cmd_bus;
mod cmd_bus_ports;
mod cmd_connections;
mod cmd_devices;
mod cmd_gaps;
mod cmd_generic;
mod cmd_ip_box;
mod cmd_items;
mod cmd_join_device;
mod cmd_layout;
mod cmd_library_part1;
mod cmd_pages;
mod cmd_part_edit;
mod cmd_parts;
mod cmd_paste_adapt;
mod cmd_place_arrange;
mod cmd_plc_normalisers;
mod cmd_plc_part1;
mod cmd_project;
mod cmd_revision;
mod cmd_set_many;
mod cmd_settings;
mod cmd_waiver;
mod designate;
mod engine;
mod golden_dsl;
mod history;
mod history_budget;
mod known_gaps;
mod limits;
mod params;
mod props;
mod proxies;
mod review_core;
mod review_medium;
mod review_regressions;
mod terminal_insert_move;
