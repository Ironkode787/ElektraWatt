// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The 2D canvas renderer: the CPU tile path and the wgpu path over one display list (WP-S2.02).
//!
//! The CPU tile path is the GPU-less fallback and the loading state while a GPU scene prepares (FR-UI-028,
//! NFR-PLAT-03); the GPU path ([`gpu`]) draws wherever the switch rule allows. Toolkit-neutral: no egui type appears
//! here; the egui host (`ew-ui`) feeds the view in and paints what comes out, and names wgpu types only through
//! this crate's re-export [`wgpu`] (one wgpu in the graph: the version egui-wgpu locks).
//!
//! - [`view`]: the view transform from sheet nanometres to physical pixels, exact while the view moves and snapped
//!   to whole pixels once it has settled.
//! - [`tiles`]: the tile pyramid (levels `6 × 2^k` dpi), the per-frame plan (what to draw coarse to fine, what to
//!   rasterise in priority order, when the view has settled) and the tile cache with its LRU budget.
//! - [`pool`]: the raster worker pool over `ew_render_cpu::render_region`, dropping jobs a newer plan no longer
//!   wants.
//! - [`overlay`]: the page bounds, the surround, the grid dots and the hover and selection outlines as screen-space
//!   pixel rectangles, and their compositing into an image for the headless snapshot.
//! - [`stats`]: the frame statistics ring and the one clock read of the canvas.
//! - [`gpu`]: the wgpu renderer, the scene preparation worker and the switch rule.
//!
//! The CPU raster is the golden reference: tiles are regions of `ew_render_cpu`'s full-sheet raster, and the
//! headless snapshot is one such region with the overlays composited by integer pixel fills, so equal inputs give
//! equal bytes on every run of one build target (WP-S2.02 §8).

#![forbid(unsafe_code)]

pub use wgpu;

pub mod gpu;
pub mod overlay;
pub mod pool;
pub mod stats;
pub mod tiles;
pub mod view;
