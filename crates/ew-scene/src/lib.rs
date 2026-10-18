// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Model to display lists per page and layer, forms engine (frames, title blocks, report forms), text
//! through `ew-text`, spatial index and hit-testing.
//!
//! So far: symbol geometry lowering ([`symbol::lower`], part of WP-S3.01), the pure function behind
//! ADR-0015's memoised `symbol_geometry(symbol, variant)`: a library [`SymbolDef`](ew_schema::SymbolDef)
//! and an orientation variant give an `ew-dl` display list, the placed CPs and texts, the resolved anchor
//! and the bounds, all in the symbol's own frame.

#![forbid(unsafe_code)]

pub mod symbol;
