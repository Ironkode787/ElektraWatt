// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-api`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
#[path = "../golden_common/mod.rs"]
mod golden_common;
#[path = "../qet_synth/mod.rs"]
mod qet_synth;
#[path = "../page_output/view.rs"]
#[allow(dead_code)]
mod view;

mod bus_part4;
mod bus_part4b;
mod catalog;
mod checks_part5;
mod clipboard;
mod cmdlog;
mod connectivity_reports;
mod decimal_separator;
mod derivation;
mod editing;
mod export_bom;
mod export_job;
mod find;
mod fmt;
mod golden_c;
mod golden_connectivity;
mod golden_csv;
mod golden_fixture;
mod golden_layout;
mod golden_layout_bom;
mod golden_xref;
mod image_items;
mod images;
mod import_csv;
mod import_template_cps;
mod libraries;
mod library_hash;
mod library_navigators;
mod library_ops;
mod library_parts;
mod macros;
mod macros_valve;
mod manufacturer_copies;
mod messages;
mod navigation;
mod navigators;
mod navigators_part2;
mod new_project;
mod page_items;
mod page_output_memo;
mod page_output_panel;
mod part_selection;
mod parts_import;
mod persistence;
mod plc_navigator;
mod plc_part3;
mod plc_texts;
mod plc_window;
mod preview_history;
mod property_grid;
mod qet_import;
mod qet_lenient;
mod qet_private_corpus;
mod queries;
mod raster_exporters;
mod recovery;
mod release;
mod release_gate;
mod render;
mod report_pages;
mod reports_golden;
mod revision_status;
mod session;
mod sql_projection;
mod store_items;
mod strip_edit;
mod text_styles;
mod verify;
mod wiring;
