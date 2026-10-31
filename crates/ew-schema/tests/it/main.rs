// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The integration tests of `ew-schema`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod bus_records;
mod exporters;
mod layout_records;
mod part1_members;
mod part_bus_section;
mod props;
mod records;
mod registry;
mod rejections;
mod revision_labels;
mod schema_validation;
mod schemas;
mod structured;
mod text_styles;
mod unknown_members;
