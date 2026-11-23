// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page as display lists per layer ([`page`]), the forms engine (frames, title blocks, report forms), text
//! through `ew-text`. An item drawn for an entity carries it, which the window's hit-testing reads; the crate keeps no
//! spatial index.
//!
//! Symbol geometry lowering ([`symbol::lower`]) is the pure function behind
//! ADR-0015's memoised `symbol_geometry(symbol, variant)`: a library [`SymbolDef`](ew_schema::SymbolDef)
//! and an orientation variant give an `ew-dl` display list, the placed CPs and texts, the resolved anchor
//! and the bounds, all in the symbol's own frame.
//!
//! [`forms`]: frame geometry from the frame grid, the page-to-sheet transform
//! ([`forms::SheetMap`], FR-PAG-029), placeholder bindings and the lowering of frame and title-block forms
//! to the `frame` layer ([`forms::lower_form`]), with text shaped by `ew-text`.
//!
//! [`embedded`] draws the embedded report blocks of a drawn page into the `tables` layer (FR-REP-028): the cover's
//! colour and standard-size tables, and a boxed report table for every other kind (PART DATA, the layout legend).

#![forbid(unsafe_code)]

mod draft;
pub mod embedded;
pub mod forms;
pub mod hops;
pub mod images;
pub mod items;
pub mod knockout;
pub mod page;
pub mod panel;
pub mod pdf_text;
pub mod route;
pub mod symbol;
pub mod table;
pub mod ticks;
pub mod underlay;
