// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-crate tests: they see the crate-private internals (tables, scopes) and share the `mini_golden()`
//! fixture without a testkit dependency.

mod assembly_links;
mod content_diff;
mod field_mask;
pub(crate) mod fixture;
mod invariants;
mod ops;
mod plc_links;
mod props;
