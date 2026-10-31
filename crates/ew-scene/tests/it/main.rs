// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-scene`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod canvas_texts;
mod embedded;
mod forms_content;
mod forms_lowering;
mod forms_overflow;
mod forms_robustness;
mod frame_geometry;
mod items;
mod lowering;
mod page_scene;
mod panel_knockout;
mod pin_texts;
mod properties;
mod route;
mod slot_collisions;
mod snapshot;
mod starter_symbols;
mod symbol_text_variants;
mod tables;
mod text_styles;
