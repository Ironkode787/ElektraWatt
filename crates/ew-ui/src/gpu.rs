// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU host of the canvas (WP-S2.02 part 2, spec sections 7.1 and 7.2): which renderer draws, the GPU canvas
//! on eframe's device, the scene preparation worker, and the canvas texture registered with egui-wgpu's renderer.
//!
//! **The switch rule** is `ew_render2d::gpu::choose`, evaluated every frame from the host's facts: `--no-gpu`, the
//! setting (View, Use the CPU renderer for the canvas, effective at once), the adapter of eframe's render state
//! (none in the headless mode and in kittest: the CPU canvas), and how the GPU canvas fared: an initialisation
//! failure, a lost device or an uncaptured error moves the canvas to the CPU for the rest of the session, each with
//! one banner. The result is set with `ShellModel::set_canvas_backend` and shows in the status bar.
//!
//! **The texture.** The GPU canvas renders into its own multisampled attachments resolved into a texture of the
//! pane's physical size, registered once per size as an egui native texture (`Rgba8Unorm`, nearest filtering: one
//! texel is one physical pixel) and painted as an image over the sheet's rectangle, under the overlays that both
//! backends share. While a page's scene prepares on the worker, the CPU tiles are the loading state.
//!
//! [`wgpu_configuration`] is the window's wgpu setup: with `--no-gpu`, eframe's adapter selector picks the first
//! adapter whose device type is CPU (WARP, llvmpipe, lavapipe), else the default adapter with a banner.

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::TextureId;
pub use ew_render2d::gpu::{AdapterSummary, DeviceKind, Flags, GpuApi, GpuError, adapter_count};
use ew_render2d::gpu::{
    BackendSetting, CanvasBackend as Choice, GpuCanvas, GpuView, PreparedScene, Preparer, RenderOutcome,
    choose, no_gpu_adapter, sample_count_for,
};
use ew_render2d::pool::SharedFonts;
use ew_render2d::wgpu;
use ew_viewmodel::canvas::{CanvasBackend, CanvasPage};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

/// The window's wgpu setup (eframe's `wgpu_options`): eframe's defaults, and with `--no-gpu` the adapter selector
/// of the switch rule's row 1. The backends are the ones the build compiles (FT-328: DX12 on Windows, Metal on
/// macOS, Vulkan and GL on Linux).
pub fn wgpu_configuration(flags: Flags) -> egui_wgpu::WgpuConfiguration {
    let mut config = egui_wgpu::WgpuConfiguration::default();
    if flags.no_gpu
        && let egui_wgpu::WgpuSetup::CreateNew(create) = &mut config.wgpu_setup
    {
        create.native_adapter_selector = Some(Arc::new(no_gpu_selector));
    }
    config
}

/// The `--no-gpu` adapter selector: the first CPU-type adapter, else the default one; an error without any
/// adapter (the application then exits with code 3).
fn no_gpu_selector(
    adapters: &[wgpu::Adapter],
    _surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    let summaries: Vec<AdapterSummary> = adapters
        .iter()
        .map(|a| AdapterSummary::of(&a.get_info()))
        .collect();
    no_gpu_adapter(&summaries)
        .and_then(|(i, _)| adapters.get(i).cloned())
        .ok_or_else(|| "no graphics adapter was found".to_owned())
}

/// The status label's backend for a choice of the switch rule.
fn label_of(choice: &Choice) -> CanvasBackend {
    match choice {
        Choice::Cpu => CanvasBackend::Cpu,
        Choice::Warp => CanvasBackend::Warp,
        Choice::Gpu(api) => CanvasBackend::Gpu(api.clone()),
    }
}

/// What identifies a page's content: the project session (revisions restart with every session), the revision and
/// the folio.
#[derive(Clone, PartialEq, Eq, Debug)]
struct PageKey {
    session: u64,
    rev: u64,
    folio: String,
}

impl PageKey {
    fn of(page: &CanvasPage) -> PageKey {
        PageKey {
            session: page.session,
            rev: page.rev,
            folio: page.folio.clone(),
        }
    }
}

/// The GPU side of the canvas host.
pub struct GpuHost {
    render_state: Option<egui_wgpu::RenderState>,
    flags: Flags,
    adapter: Option<AdapterSummary>,
    /// The initialisation result a test stands in for a device with (no render state).
    simulated_init: Option<Result<(), GpuError>>,
    /// `None` until the GPU canvas was first needed.
    init: Option<Result<(), GpuError>>,
    /// A lost device or an uncaptured error after start: the CPU canvas for the rest of the session.
    fault: Option<GpuError>,
    canvas: Option<GpuCanvas>,
    preparer: Option<Preparer>,
    texture: Option<TextureId>,
    /// The page asked of the worker and its scene number.
    requested: Option<(PageKey, u64)>,
    /// The session and folio of the scene the GPU canvas holds, and its number.
    shown: Option<(u64, String, u64)>,
    next_scene: u64,
    choice: Choice,
    /// The banners already shown (each failure is told once).
    told: BTreeSet<&'static str>,
}

impl Default for GpuHost {
    fn default() -> GpuHost {
        GpuHost::new()
    }
}

impl std::fmt::Debug for GpuHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuHost")
            .field("adapter", &self.adapter)
            .field("choice", &self.choice)
            .finish_non_exhaustive()
    }
}

impl GpuHost {
    /// A host without a render state: the CPU canvas (headless, kittest).
    pub fn new() -> GpuHost {
        GpuHost {
            render_state: None,
            flags: Flags::default(),
            adapter: None,
            simulated_init: None,
            init: None,
            fault: None,
            canvas: None,
            preparer: None,
            texture: None,
            requested: None,
            shown: None,
            next_scene: 1,
            choice: Choice::Cpu,
            told: BTreeSet::new(),
        }
    }

    /// The GPU side of a second canvas group (WP-S2.03 §6.8, §9: one offscreen target per visible group): the
    /// leader's render state, flags and adapter; its switch-rule result is mirrored each frame by
    /// [`GpuHost::follow`], so the rule runs and tells its banners once.
    pub fn follower(&self) -> GpuHost {
        GpuHost {
            render_state: self.render_state.clone(),
            flags: self.flags,
            adapter: self.adapter.clone(),
            ..GpuHost::new()
        }
    }

    /// Mirrors the leader's choice for this frame: the canvas of this group is made on the leader's device when the
    /// GPU draws and released when it does not. A failure of this group's canvas is returned for the leader (which
    /// then records it as its fault: the CPU canvas for the rest of the session).
    pub fn follow(&mut self, leader: &GpuHost) -> Option<GpuError> {
        self.choice = leader.choice.clone();
        if !leader.drawing() {
            if self.canvas.is_some() {
                self.release();
                self.canvas = None;
                self.preparer = None;
            }
            return None;
        }
        if self.canvas.is_none()
            && let Some(rs) = &self.render_state
        {
            match GpuCanvas::new(rs.device.clone(), rs.queue.clone(), sample_count_for(&rs.adapter)) {
                Ok(c) => self.canvas = Some(c),
                Err(e) => return Some(e),
            }
        }
        let fault = self
            .canvas
            .as_ref()
            .and_then(GpuCanvas::fault)
            .or(self.fault.take());
        if fault.is_some() {
            self.release_frame();
            self.canvas = None;
            self.preparer = None;
            self.shown = None;
            self.requested = None;
        }
        fault
    }

    /// eframe's render state (`None`: no wgpu, the CPU canvas) and the command line's flags.
    pub fn attach(&mut self, render_state: Option<egui_wgpu::RenderState>, flags: Flags) {
        self.adapter = render_state
            .as_ref()
            .map(|rs| AdapterSummary::of(&rs.adapter.get_info()));
        self.render_state = render_state;
        self.flags = flags;
    }

    /// Stands in for a device (tests): the adapter and the result its initialisation would have.
    pub fn simulate(&mut self, adapter: AdapterSummary, init: Result<(), GpuError>) {
        self.adapter = Some(adapter);
        self.simulated_init = Some(init);
    }

    /// Records a lost device or an uncaptured error (tests stand in for the device's callbacks with it).
    pub fn inject_fault(&mut self, e: GpuError) {
        self.fault.get_or_insert(e);
    }

    /// The renderer the switch rule chose in the last frame.
    pub fn choice(&self) -> &Choice {
        &self.choice
    }

    /// Whether the GPU canvas draws (the rule chose it and the canvas exists).
    pub fn drawing(&self) -> bool {
        self.choice.is_gpu() && self.canvas.is_some()
    }

    /// The GPU canvas (tests and the statistics).
    pub fn canvas(&self) -> Option<&GpuCanvas> {
        self.canvas.as_ref()
    }

    fn init(&mut self) -> Result<(), GpuError> {
        let Some(rs) = &self.render_state else {
            return self.simulated_init.clone().unwrap_or(Ok(()));
        };
        let canvas = GpuCanvas::new(rs.device.clone(), rs.queue.clone(), sample_count_for(&rs.adapter))?;
        self.canvas = Some(canvas);
        Ok(())
    }

    /// Lets go of the scene as well as the frame (no project, the wizard, the start page): nothing of the page on
    /// show may stand in for the next page, which may be another project's at an equal revision and folio.
    pub fn release(&mut self) {
        self.release_frame();
        self.requested = None;
        self.shown = None;
        if let Some(c) = &mut self.canvas {
            c.clear_scene();
        }
    }

    /// Puts a GPU canvas into the host without a render state (tests: a headless device's canvas); the switch rule
    /// is not asked.
    #[doc(hidden)]
    pub fn set_canvas_for_tests(&mut self, canvas: GpuCanvas) {
        self.canvas = Some(canvas);
    }

    /// The scene number last asked of the worker (tests).
    #[doc(hidden)]
    pub fn requested_scene(&self) -> Option<u64> {
        self.requested.as_ref().map(|r| r.1)
    }

    /// The latch of the preparation worker (tests: hold it to observe the host while a scene prepares, without
    /// timing). The worker is made here when the host has none yet.
    #[doc(hidden)]
    pub fn preparation_hold(&mut self, ctx: &egui::Context) -> Arc<ew_render2d::gpu::Hold> {
        Arc::clone(self.preparer(ctx).hold())
    }

    /// Blocks until the worker sends a scene (at most `limit`) and takes it as `sync_scene` would (tests).
    #[doc(hidden)]
    pub fn await_scene_for_tests(&mut self, page: &CanvasPage, limit: std::time::Duration) {
        let received = self.preparer.as_ref().and_then(|p| p.recv_timeout(limit));
        if let Some(prepared) = received {
            self.accept(prepared, page);
        }
    }

    fn preparer(&mut self, ctx: &egui::Context) -> &Preparer {
        self.preparer.get_or_insert_with(|| {
            let ctx = ctx.clone();
            Preparer::new(Arc::new(move || ctx.request_repaint()))
        })
    }

    /// Hands a prepared scene to the host as the worker would (tests: a late, older scene).
    #[doc(hidden)]
    pub fn deliver_for_tests(&mut self, prepared: Arc<PreparedScene>, page: &CanvasPage) {
        self.accept(prepared, page);
    }

    /// Lets go of the canvas texture and the render targets (the canvas is not on screen or the CPU draws).
    pub fn release_frame(&mut self) {
        if let (Some(id), Some(rs)) = (self.texture.take(), &self.render_state) {
            rs.renderer.write().free_texture(&id);
        }
        if let Some(c) = &mut self.canvas {
            c.release_targets();
        }
    }

    /// Applies the switch rule for this frame and sets the shell's backend label; a failure is told once in the
    /// banner.
    pub fn update(&mut self, shell: &mut ShellModel) {
        let setting = if shell.cpu_canvas_setting() {
            BackendSetting::Cpu
        } else {
            BackendSetting::Auto
        };
        if self.init.is_none() && choose(self.flags, setting, self.adapter.as_ref(), &Ok(())).is_gpu() {
            self.init = Some(self.init());
        }
        if let Some(fault) = self.canvas.as_ref().and_then(GpuCanvas::fault) {
            self.fault.get_or_insert(fault);
        }
        if self.fault.is_some() && self.canvas.is_some() {
            self.release_frame();
            self.canvas = None;
            self.preparer = None;
            self.shown = None;
            self.requested = None;
        }
        let status = match (&self.fault, &self.init) {
            (Some(f), _) => Err(f.clone()),
            (None, Some(r)) => r.clone(),
            (None, None) => Ok(()),
        };
        let choice = choose(self.flags, setting, self.adapter.as_ref(), &status);
        if !choice.is_gpu() && self.choice.is_gpu() {
            self.release_frame();
        }
        if let Some(Err(e)) = &self.init
            && self.told.insert("init")
        {
            shell.show_banner(&strings::format("banner-gpu-init", &[&e.to_string()]));
        }
        if let Some(f) = &self.fault
            && self.told.insert("fault")
        {
            shell.show_banner(&strings::format("banner-gpu-lost", &[&f.to_string()]));
        }
        if self.flags.no_gpu
            && let Some(a) = &self.adapter
            && a.kind != DeviceKind::Cpu
            && self.told.insert("no-software-adapter")
        {
            shell.show_banner(&strings::format("banner-gpu-no-software-adapter", &[&a.name]));
        }
        let label = label_of(&choice);
        if shell.canvas_backend() != &label {
            shell.set_canvas_backend(label);
        }
        self.choice = choice;
    }

    /// Asks the worker for the scene of `page` once per (revision, folio) and takes a finished one: `true` when
    /// the GPU canvas holds a scene of this folio (possibly of an older revision), with `ready` telling whether it
    /// is the page's current revision.
    pub fn sync_scene(
        &mut self,
        ctx: &egui::Context,
        page: &Arc<CanvasPage>,
        fonts: &SharedFonts,
    ) -> (bool, bool) {
        let Some(canvas) = &mut self.canvas else {
            return (false, false);
        };
        let preparer = self.preparer.get_or_insert_with(|| {
            let ctx = ctx.clone();
            Preparer::new(Arc::new(move || ctx.request_repaint()))
        });
        let key = PageKey::of(page);
        if self.requested.as_ref().is_none_or(|(k, _)| *k != key) {
            let id = self.next_scene;
            self.next_scene += 1;
            preparer.submit(id, Arc::clone(&page.list), Arc::clone(fonts));
            self.requested = Some((key, id));
            if self
                .shown
                .as_ref()
                .is_some_and(|(s, f, _)| *s != page.session || *f != page.folio)
            {
                // Another page, or a page of another session: its old scene must not stand in.
                canvas.clear_scene();
                self.shown = None;
            }
        }
        if let Some(prepared) = preparer.try_recv() {
            self.accept(prepared, page);
        }
        let current = self.requested.as_ref().map(|r| r.1);
        match &self.shown {
            Some((session, folio, id)) if *session == page.session && *folio == page.folio => {
                (true, Some(*id) == current)
            }
            _ => (false, false),
        }
    }

    fn accept(&mut self, prepared: Arc<PreparedScene>, page: &CanvasPage) {
        let Some(canvas) = &mut self.canvas else { return };
        let current = self.requested.as_ref().map(|r| r.1);
        // Only the scene last asked for is shown; an older one that arrives late is dropped.
        if Some(prepared.id) != current {
            return;
        }
        self.shown = Some((page.session, page.folio.clone(), prepared.id));
        canvas.set_scene(prepared);
    }

    /// Renders the scene at `view` (only when something changed) and returns the texture to paint; the texture is
    /// registered with egui-wgpu's renderer once per size. An error is recorded as a fault (the CPU canvas from the
    /// next frame on).
    pub fn render(&mut self, view: &GpuView) -> Option<(TextureId, RenderOutcome)> {
        let canvas = self.canvas.as_mut()?;
        let outcome = match canvas.render(view) {
            Ok(o) => o,
            Err(e) => {
                self.fault.get_or_insert(e);
                return None;
            }
        };
        let rs = self.render_state.as_ref()?;
        let texture_view = canvas.texture_view()?;
        let filter = wgpu::FilterMode::Nearest;
        match self.texture {
            Some(id) if outcome.resized => {
                rs.renderer.write().update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    texture_view,
                    filter,
                    id,
                );
            }
            Some(_) => {}
            None => {
                self.texture = Some(rs.renderer.write().register_native_texture(
                    &rs.device,
                    texture_view,
                    filter,
                ));
            }
        }
        self.texture.map(|id| (id, outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_label_follows_the_choice() {
        assert_eq!(label_of(&Choice::Cpu), CanvasBackend::Cpu);
        assert_eq!(label_of(&Choice::Warp).label(), "WARP");
        assert_eq!(label_of(&Choice::Gpu("Vulkan".into())).label(), "GPU Vulkan");
    }

    #[test]
    fn the_window_setup_has_a_selector_only_with_no_gpu() {
        let selector = |c: &egui_wgpu::WgpuConfiguration| match &c.wgpu_setup {
            egui_wgpu::WgpuSetup::CreateNew(n) => n.native_adapter_selector.is_some(),
            egui_wgpu::WgpuSetup::Existing(_) => false,
        };
        assert!(!selector(&wgpu_configuration(Flags::default())));
        assert!(selector(&wgpu_configuration(Flags { no_gpu: true })));
        assert_eq!(
            no_gpu_selector(&[], None).err().as_deref(),
            Some("no graphics adapter was found")
        );
    }
}
