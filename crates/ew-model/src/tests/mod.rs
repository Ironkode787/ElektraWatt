// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-crate tests: they see the crate-private internals (tables, scopes) and share the `mini_project()`
//! fixture of `crate::fixture`.

mod assembly_links;
mod assets;
mod content_diff;
mod field_mask;
mod invariants;
mod layout_links;
mod macro_links;
mod ops;
mod plc_links;
mod props;
