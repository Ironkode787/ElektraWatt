// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! CPU renderer: GPU-less fallback, thumbnails, golden PNGs and banded printing.
//!
//! [`render`] rasterises an [`ew_dl::DisplayList`] (integer nanometres on the sheet, origin top-left,
//! y down) onto an RGBA8 [`RgbaImage`] of [`pixel_size`] pixels with `vello_cpu`: paths fill
//! (non-zero) then stroke with their true widths (hairlines are anti-aliased, never clamped), glyph
//! runs draw their outlines from the font bytes a [`FontResolver`] supplies (unhinted; never system
//! fonts), groups transform and clip, and instances are drawn through a
//! [`GeometryResolver`](ew_dl::GeometryResolver). What it had to skip (missing fonts, unresolved
//! instances, images) comes back in the [`RenderReport`], never as a panic.
//!
//! The rasteriser runs single-threaded on the `Level::baseline` u8 pipeline, so the same list, fonts
//! and options give byte-identical pixels on every run of one build target (doc 09 §4.4). Glyph
//! caches live inside one call and cannot carry state into the next.
//!
//! [`render_region`] rasterises one rectangle of that full-sheet image (the canvas tiles and the headless canvas
//! snapshot of WP-S2.02), skipping top-level items whose [`ItemBounds`] miss it; its full rectangle is
//! [`render`]'s bytes.
//!
//! [`encode_png`] writes deterministic PNG bytes; [`monochrome`] and [`render_monochrome`] give the
//! black-and-grey form for monochrome output (FR-OUT-011).

#![forbid(unsafe_code)]

mod fonts;
mod mono;
mod png;
mod render;

pub use crate::png::encode_png;
pub use fonts::{FontBytes, FontResolver};
pub use mono::{monochrome, render_monochrome};
pub use render::{
    ItemBounds, RegionOptions, RenderError, RenderOptions, RenderReport, RgbaImage, SheetSize, pixel_size,
    render, render_region,
};
