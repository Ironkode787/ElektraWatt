// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A headless device for the render-to-texture tests and the bench (WP-S2.02 §8 item 4, §9): no window, no
//! surface. The window's device is eframe's (one wgpu device, ADR-0014); this one only exists where there is no
//! window.

use super::render::{GpuCanvas, sample_count_for};
use super::switch::{AdapterSummary, GpuError};

/// A device without a window, with its adapter's summary.
pub struct HeadlessDevice {
    /// The adapter.
    pub adapter: wgpu::Adapter,
    /// The device.
    pub device: wgpu::Device,
    /// Its queue.
    pub queue: wgpu::Queue,
    /// What the switch rule knows about the adapter.
    pub summary: AdapterSummary,
}

impl std::fmt::Debug for HeadlessDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeadlessDevice")
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

impl HeadlessDevice {
    /// A device on the default adapter of the compiled backends (wgpu's `WGPU_BACKEND` and `WGPU_ADAPTER_NAME`
    /// apply), or on the fallback (software) adapter when there is no other: WARP on Windows, lavapipe or
    /// llvmpipe where Mesa is installed. `None` without any adapter.
    pub fn new() -> Option<HeadlessDevice> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let request = |fallback: bool| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: fallback,
                compatible_surface: None,
                apply_limit_buckets: false,
            }))
            .ok()
        };
        let adapter = request(false).or_else(|| request(true))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("ew-canvas-headless"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..wgpu::DeviceDescriptor::default()
        }))
        .ok()?;
        let summary = AdapterSummary::of(&adapter.get_info());
        Some(HeadlessDevice {
            adapter,
            device,
            queue,
            summary,
        })
    }

    /// A canvas on this device, multisampled as the adapter allows.
    ///
    /// # Errors
    ///
    /// The canvas's initialisation error.
    pub fn canvas(&self) -> Result<GpuCanvas, GpuError> {
        GpuCanvas::new(
            self.device.clone(),
            self.queue.clone(),
            sample_count_for(&self.adapter),
        )
    }
}
