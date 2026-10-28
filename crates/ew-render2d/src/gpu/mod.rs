// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU path of the canvas (WP-S2.02 part 2, §7): the display list tessellated with lyon, drawn instanced with a
//! depth-ordered paint order and screen-space minimum stroke widths into an offscreen multisampled texture, which
//! the egui host shows as a native texture under the overlays of [`crate::overlay`].
//!
//! - [`switch`]: the switch rule as the pure function [`choose`], and the `--no-gpu` adapter selector.
//! - [`prepare`](mod@prepare): scene preparation on a worker ([`Preparer`]): meshes keyed by content, glyph meshes by (font key,
//!   glyph ID), instances, LOD bars, raster images decoded (and downscaled to the device's limit) as
//!   [`ImageTexture`]s; the [`MeshCache`] across scenes.
//! - [`tess`]: the fills and strokes with lyon, dashes with kurbo.
//! - [`render`]: the [`GpuCanvas`]: pipelines (meshes, and textured quads for raster images), uploads, the texture
//!   cache, the render on demand and the test readback.
//! - [`device`]: a headless device for the render-to-texture tests and the bench.
//!
//! The CPU raster stays the golden reference (§8): no GPU image reaches a golden, a transcript or a describe text.

pub mod device;
pub mod prepare;
pub mod render;
pub mod switch;
pub mod tess;

pub use device::{HeadlessDevice, adapter_count};
pub use prepare::{
    DEFAULT_MAX_IMAGE_SIDE, Hold, ImageInstance, ImageTexture, MeshCache, PreparedScene, Preparer, fit_side,
    image_texture, prepare,
};
pub use render::{GPU_TEXTURE_BYTES, GpuCanvas, GpuView, RenderOutcome, TARGET_FORMAT, sample_count_for};
pub use switch::{
    AdapterSummary, BackendSetting, CanvasBackend, DeviceKind, Flags, GpuApi, GpuError, choose,
    no_gpu_adapter,
};
