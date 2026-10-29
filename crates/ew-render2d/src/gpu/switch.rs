// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The switch rule (WP-S2.02 §7.2): which renderer draws the canvas, as one pure function over the host's facts.
//!
//! | # | Condition | Canvas |
//! |---|---|---|
//! | 1 | `--no-gpu` | CPU |
//! | 2 | the setting `canvas_backend = "cpu"` | CPU |
//! | 3 | no wgpu render state (headless, kittest) | CPU |
//! | 6 | a software adapter on a backend other than DX12 (llvmpipe, lavapipe) | CPU |
//! | 7, 8 | the GPU canvas failed to initialise, or lost its device or met an uncaptured error after start | CPU |
//! | 5 | the CPU device type on DX12 (WARP) | GPU, labelled `WARP` |
//! | 4 | a discrete, integrated, virtual or other adapter | GPU, labelled `GPU <backend>` |
//!
//! The rows are checked in that order: an adapter that row 6 rules out is never initialised, so its `init` result
//! does not matter. A host with no adapter at all cannot open its window (egui's own chrome needs one); the
//! application exits with code 3 before the rule is ever asked (decision log ID-514).

use std::fmt;

/// The kind of device an adapter drives (`wgpu::DeviceType`, without the wgpu type, so that tests build summaries
/// of adapters that do not exist).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DeviceKind {
    /// A discrete GPU.
    Discrete,
    /// An integrated GPU.
    Integrated,
    /// A virtual or hosted GPU.
    Virtual,
    /// A device of unknown kind.
    Other,
    /// A software rasteriser: WARP on DX12, llvmpipe or lavapipe on GL and Vulkan.
    Cpu,
}

/// The graphics API an adapter is driven through.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GpuApi {
    /// Direct3D 12 (Windows).
    Dx12,
    /// Metal (macOS).
    Metal,
    /// Vulkan.
    Vulkan,
    /// OpenGL or OpenGL ES.
    Gl,
    /// Anything else (the no-op backend, WebGPU).
    Other,
}

impl GpuApi {
    /// The name in the status label: `DX12`, `Metal`, `Vulkan`, `GL`.
    pub fn label(self) -> &'static str {
        match self {
            GpuApi::Dx12 => "DX12",
            GpuApi::Metal => "Metal",
            GpuApi::Vulkan => "Vulkan",
            GpuApi::Gl => "GL",
            GpuApi::Other => "other",
        }
    }
}

/// What the switch rule needs to know about an adapter.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AdapterSummary {
    /// The device kind.
    pub kind: DeviceKind,
    /// The graphics API.
    pub api: GpuApi,
    /// The adapter's name as the driver reports it (shown in banners only).
    pub name: String,
}

impl AdapterSummary {
    /// The summary of a wgpu adapter's information.
    pub fn of(info: &wgpu::AdapterInfo) -> AdapterSummary {
        let kind = match info.device_type {
            wgpu::DeviceType::DiscreteGpu => DeviceKind::Discrete,
            wgpu::DeviceType::IntegratedGpu => DeviceKind::Integrated,
            wgpu::DeviceType::VirtualGpu => DeviceKind::Virtual,
            wgpu::DeviceType::Cpu => DeviceKind::Cpu,
            wgpu::DeviceType::Other => DeviceKind::Other,
        };
        let api = match info.backend {
            wgpu::Backend::Dx12 => GpuApi::Dx12,
            wgpu::Backend::Metal => GpuApi::Metal,
            wgpu::Backend::Vulkan => GpuApi::Vulkan,
            wgpu::Backend::Gl => GpuApi::Gl,
            _ => GpuApi::Other,
        };
        AdapterSummary {
            kind,
            api,
            name: info.name.clone(),
        }
    }

    /// Whether this is DX12's software adapter (WARP).
    pub fn is_warp(&self) -> bool {
        self.kind == DeviceKind::Cpu && self.api == GpuApi::Dx12
    }
}

/// The command-line facts of the switch rule.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Flags {
    /// `--no-gpu` was given.
    pub no_gpu: bool,
}

/// The `canvas_backend` setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BackendSetting {
    /// The switch rule decides (the default).
    #[default]
    Auto,
    /// Always the CPU canvas (View, Use the CPU renderer for the canvas).
    Cpu,
}

/// Why the GPU canvas cannot draw.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GpuError {
    /// Creating the pipelines or the first resources failed (a validation or memory error caught in an error scope).
    Init(String),
    /// The device was lost after start.
    DeviceLost(String),
    /// An error no scope caught, after start.
    Uncaptured(String),
    /// A render target or buffer the canvas needs exceeds the device's limits.
    Limits(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuError::Init(m) => write!(f, "the GPU canvas could not be initialised: {m}"),
            GpuError::DeviceLost(m) => write!(f, "the graphics device was lost: {m}"),
            GpuError::Uncaptured(m) => write!(f, "the graphics device reported an error: {m}"),
            GpuError::Limits(m) => write!(f, "the canvas exceeds the graphics device's limits: {m}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Where the canvas draws (the status label of `ew-viewmodel`'s backend, which the host maps it to).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CanvasBackend {
    /// The CPU tile path.
    Cpu,
    /// The GPU path on DX12's software adapter.
    Warp,
    /// The GPU path on a hardware adapter, with the API's label (`DX12`, `Metal`, `Vulkan`, `GL`).
    Gpu(String),
}

impl CanvasBackend {
    /// Whether the GPU path draws.
    pub fn is_gpu(&self) -> bool {
        !matches!(self, CanvasBackend::Cpu)
    }
}

/// The switch rule of WP-S2.02 §7.2 (see the module documentation for the rows and their order).
pub fn choose(
    flags: Flags,
    setting: BackendSetting,
    adapter: Option<&AdapterSummary>,
    init: &Result<(), GpuError>,
) -> CanvasBackend {
    if flags.no_gpu || setting == BackendSetting::Cpu {
        return CanvasBackend::Cpu;
    }
    let Some(adapter) = adapter else {
        return CanvasBackend::Cpu;
    };
    if adapter.kind == DeviceKind::Cpu && adapter.api != GpuApi::Dx12 {
        return CanvasBackend::Cpu;
    }
    if init.is_err() {
        return CanvasBackend::Cpu;
    }
    if adapter.is_warp() {
        CanvasBackend::Warp
    } else {
        CanvasBackend::Gpu(adapter.api.label().to_owned())
    }
}

/// The adapter `--no-gpu` gives the window (WP-S2.02 §7.2 row 1): the first adapter whose device kind is
/// [`DeviceKind::Cpu`]; without one, the default adapter (the first discrete, then integrated, virtual, other
/// adapter, the order of a high-performance preference) and `false` (the host shows a banner). `None` for an empty
/// list (no adapter at all: exit code 3).
pub fn no_gpu_adapter(adapters: &[AdapterSummary]) -> Option<(usize, bool)> {
    if let Some(i) = adapters.iter().position(|a| a.kind == DeviceKind::Cpu) {
        return Some((i, true));
    }
    let rank = |k: DeviceKind| match k {
        DeviceKind::Discrete => 0,
        DeviceKind::Integrated => 1,
        DeviceKind::Virtual => 2,
        DeviceKind::Other => 3,
        DeviceKind::Cpu => 4,
    };
    (0..adapters.len())
        .min_by_key(|&i| (rank(adapters[i].kind), i))
        .map(|i| (i, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(kind: DeviceKind, api: GpuApi) -> AdapterSummary {
        AdapterSummary {
            kind,
            api,
            name: "test adapter".to_owned(),
        }
    }

    const OK: Result<(), GpuError> = Ok(());

    fn gpu(label: &str) -> CanvasBackend {
        CanvasBackend::Gpu(label.to_owned())
    }

    #[test]
    fn b1_every_row_of_the_switch_rule() {
        let auto = BackendSetting::Auto;
        let none = Flags::default();
        let vulkan = adapter(DeviceKind::Discrete, GpuApi::Vulkan);
        let warp = adapter(DeviceKind::Cpu, GpuApi::Dx12);
        let lavapipe = adapter(DeviceKind::Cpu, GpuApi::Vulkan);
        let llvmpipe = adapter(DeviceKind::Cpu, GpuApi::Gl);
        // Row 1: --no-gpu, whatever the adapter.
        let no_gpu = Flags { no_gpu: true };
        for a in [&vulkan, &warp] {
            assert_eq!(choose(no_gpu, auto, Some(a), &OK), CanvasBackend::Cpu);
        }
        // Row 2: the setting.
        assert_eq!(
            choose(none, BackendSetting::Cpu, Some(&vulkan), &OK),
            CanvasBackend::Cpu
        );
        // Row 3: no render state.
        assert_eq!(choose(none, auto, None, &OK), CanvasBackend::Cpu);
        // Row 4: discrete, integrated, virtual and other adapters, labelled by their API.
        assert_eq!(choose(none, auto, Some(&vulkan), &OK), gpu("Vulkan"));
        for (kind, api, label) in [
            (DeviceKind::Integrated, GpuApi::Dx12, "DX12"),
            (DeviceKind::Virtual, GpuApi::Metal, "Metal"),
            (DeviceKind::Other, GpuApi::Gl, "GL"),
        ] {
            assert_eq!(choose(none, auto, Some(&adapter(kind, api)), &OK), gpu(label));
        }
        // Row 5: WARP stays on the GPU path (ID-514).
        assert_eq!(choose(none, auto, Some(&warp), &OK), CanvasBackend::Warp);
        // Row 6: other software adapters draw on the CPU, even when their initialisation would succeed.
        for a in [&lavapipe, &llvmpipe] {
            assert_eq!(choose(none, auto, Some(a), &OK), CanvasBackend::Cpu);
        }
        // Rows 7 and 8: a failed initialisation, a lost device, an uncaptured error.
        for e in [
            GpuError::Init("pipeline".into()),
            GpuError::DeviceLost("reset".into()),
            GpuError::Uncaptured("validation".into()),
            GpuError::Limits("texture".into()),
        ] {
            assert_eq!(
                choose(none, auto, Some(&vulkan), &Err(e.clone())),
                CanvasBackend::Cpu
            );
            assert_eq!(choose(none, auto, Some(&warp), &Err(e)), CanvasBackend::Cpu);
        }
        assert!(gpu("Vulkan").is_gpu() && CanvasBackend::Warp.is_gpu() && !CanvasBackend::Cpu.is_gpu());
    }

    #[test]
    fn b1_the_no_gpu_adapter_selector_on_synthetic_lists() {
        let hw = adapter(DeviceKind::Discrete, GpuApi::Dx12);
        let igpu = adapter(DeviceKind::Integrated, GpuApi::Dx12);
        let warp = adapter(DeviceKind::Cpu, GpuApi::Dx12);
        let other = adapter(DeviceKind::Other, GpuApi::Gl);
        // The software adapter wins wherever it is in the list.
        assert_eq!(
            no_gpu_adapter(&[hw.clone(), igpu.clone(), warp.clone()]),
            Some((2, true))
        );
        assert_eq!(no_gpu_adapter(&[warp.clone(), hw.clone()]), Some((0, true)));
        // None: the default adapter, flagged for the banner.
        assert_eq!(no_gpu_adapter(&[igpu.clone(), hw.clone()]), Some((1, false)));
        assert_eq!(no_gpu_adapter(&[other.clone(), igpu]), Some((1, false)));
        assert_eq!(no_gpu_adapter(&[other]), Some((0, false)));
        // No adapter at all.
        assert_eq!(no_gpu_adapter(&[]), None);
    }
}
