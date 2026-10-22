// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page view (WP-S2.01 part 1, spec section 5.3, decision log ID-445): the current page as the CPU renderer's
//! image, read-only, fitted to the pane with its aspect ratio kept.
//!
//! 1. On the UI thread, [`ShellModel::page_content`] gives the page's sheet display list through the session's memo
//!    store (the `Session` is not `Send`).
//! 2. One worker thread (a latest-request-wins slot, so a held Page Down key does not stack renders) runs [`render_page`] and sends the image back over a channel, tagged with
//!    `(revision, folio, dpi)`. The UI uploads it as a texture; a result whose tag no longer matches is dropped.
//! 3. The display dpi is the largest multiple of 12 between 36 and 200 at which the sheet fits the pane at the
//!    current `pixels_per_point`, so a resize re-rasters only when it crosses a step.
//! 4. Until the image arrives the pane shows "Rendering page {folio}..."; a render error shows its message.
//!
//! [`render_page`] is also the path of `view.snapshot` in the headless mode.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{ColorImage, TextureHandle, TextureOptions, Ui, Vec2};
use ew_render_cpu::{RenderOptions, RgbaImage, SheetSize, pixel_size, render};
use ew_viewmodel::ew_api::output::{ExportPages, font_bytes};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::latest::LatestWorker;

/// The dpi steps of the page view: multiples of 12 from 36 to 192 (the largest multiple of 12 below 200).
const DPI_MIN: u32 = 36;
const DPI_MAX: u32 = 200;
const DPI_STEP: u32 = 12;

/// Renders the first page of `pages` at `dpi` through the CPU renderer with the bundled fonts.
///
/// # Errors
///
/// The text of the renderer's error (a sheet without extent, an image beyond the rasteriser's limit) or of a
/// missing page.
pub fn render_page(pages: &ExportPages, dpi: u32) -> Result<RgbaImage, String> {
    let page = pages
        .pages
        .first()
        .ok_or_else(|| strings::text("page-no-pages").to_owned())?;
    let sheet = SheetSize {
        width: page.width,
        height: page.height,
    };
    let options = RenderOptions {
        dpi,
        ..RenderOptions::default()
    };
    render(&page.content, sheet, &options, font_bytes(), None)
        .map(|(image, _report)| image)
        .map_err(|e| strings::format("error-render", &[&e.to_string()]))
}

/// The display dpi: the largest multiple of 12 in `36..=200` at which `pages`' sheet fits `pane_px` (the pane in
/// physical pixels), or 36 when even that does not fit.
pub fn fit_dpi(pages: &ExportPages, pane_px: Vec2) -> u32 {
    let Some(page) = pages.pages.first() else {
        return DPI_MIN;
    };
    let sheet = SheetSize {
        width: page.width,
        height: page.height,
    };
    let mut dpi = (DPI_MAX / DPI_STEP) * DPI_STEP;
    while dpi > DPI_MIN {
        let (w, h) = pixel_size(sheet, dpi);
        if w as f32 <= pane_px.x && h as f32 <= pane_px.y {
            return dpi;
        }
        dpi -= DPI_STEP;
    }
    DPI_MIN
}

/// What a rendered image is for.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Tag {
    rev: u64,
    folio: String,
    dpi: u32,
}

/// A render for the worker.
struct Request {
    tag: Tag,
    pages: Arc<ExportPages>,
    ctx: egui::Context,
}

struct Done {
    tag: Tag,
    image: Result<RgbaImage, String>,
}

struct Content {
    rev: u64,
    folio: String,
    pages: Arc<ExportPages>,
}

/// The page view's state: the phase-1 content, the request in flight and the texture on show.
pub struct PageView {
    tx: Sender<Done>,
    rx: Receiver<Done>,
    /// The one render worker (started with the first request): a request replaces one still waiting.
    worker: Option<LatestWorker<Request>>,
    content: Option<Content>,
    failed: Option<(u64, String, String)>,
    requested: Option<Tag>,
    shown: Option<(Tag, TextureHandle)>,
    error: Option<(Tag, String)>,
}

impl Default for PageView {
    fn default() -> PageView {
        PageView::new()
    }
}

impl PageView {
    /// A page view with nothing rendered.
    pub fn new() -> PageView {
        let (tx, rx) = channel();
        PageView {
            tx,
            rx,
            worker: None,
            content: None,
            failed: None,
            requested: None,
            shown: None,
            error: None,
        }
    }

    /// Paints the current page of `shell` into `ui`: the image when it is there, else the waiting text.
    pub fn show(&mut self, ui: &mut Ui, shell: &ShellModel) {
        let (Some(folio), Some(rev)) = (shell.current_folio(), shell.revision()) else {
            ui.label(strings::text("page-no-pages"));
            return;
        };
        let folio = folio.to_owned();
        self.refresh_content(shell, rev, &folio);
        if let Some((_, _, message)) = self.failed.as_ref().filter(|(r, f, _)| *r == rev && *f == folio) {
            ui.label(message.as_str());
            return;
        }
        let Some(content) = &self.content else {
            ui.label(strings::format("page-rendering", &[&folio]));
            return;
        };
        let ppp = ui.ctx().pixels_per_point();
        let avail = ui.available_size();
        let dpi = fit_dpi(&content.pages, avail * ppp);
        let tag = Tag {
            rev,
            folio: folio.clone(),
            dpi,
        };
        if self.requested.as_ref() != Some(&tag) {
            self.requested = Some(tag.clone());
            self.spawn(ui.ctx(), tag, Arc::clone(&content.pages));
        }
        self.collect(ui.ctx());
        if let Some((tag, message)) = &self.error
            && tag.rev == rev
            && tag.folio == folio
        {
            ui.label(message.as_str());
            return;
        }
        match &self.shown {
            Some((tag, texture)) if tag.rev == rev && tag.folio == folio => {
                let size = texture.size_vec2();
                let scale = (avail.x / size.x).min(avail.y / size.y);
                let size = size * scale;
                let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
                let image_rect = egui::Rect::from_center_size(rect.center(), size);
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            _ => {
                ui.label(strings::format("page-rendering", &[&folio]));
            }
        }
    }

    /// Phase 1 for the current page, once per `(revision, folio)`.
    fn refresh_content(&mut self, shell: &ShellModel, rev: u64, folio: &str) {
        let current = self
            .content
            .as_ref()
            .is_some_and(|c| c.rev == rev && c.folio == folio);
        let failed = self
            .failed
            .as_ref()
            .is_some_and(|(r, f, _)| *r == rev && f == folio);
        if current || failed {
            return;
        }
        match shell.page_content(folio) {
            Ok(pages) => {
                self.content = Some(Content {
                    rev,
                    folio: folio.to_owned(),
                    pages: Arc::new(pages),
                });
                self.failed = None;
            }
            Err(message) => {
                self.content = None;
                self.failed = Some((rev, folio.to_owned(), message));
            }
        }
    }

    fn spawn(&mut self, ctx: &egui::Context, tag: Tag, pages: Arc<ExportPages>) {
        let tx = self.tx.clone();
        let worker = self.worker.get_or_insert_with(|| {
            LatestWorker::new(move |request: Request| {
                let image = render_page(&request.pages, request.tag.dpi);
                // The receiver is gone when the window closed; nothing to do then.
                let _ = tx.send(Done {
                    tag: request.tag,
                    image,
                });
                request.ctx.request_repaint();
            })
        });
        worker.submit(Request {
            tag,
            pages,
            ctx: ctx.clone(),
        });
    }

    /// Takes the finished images: the one that matches the request is uploaded, the others are dropped.
    fn collect(&mut self, ctx: &egui::Context) {
        while let Ok(done) = self.rx.try_recv() {
            if self.requested.as_ref() != Some(&done.tag) {
                continue;
            }
            match done.image {
                Ok(image) => {
                    let size = [image.width as usize, image.height as usize];
                    let color = ColorImage::from_rgba_unmultiplied(size, &image.pixels);
                    let texture = ctx.load_texture("page-view", color, TextureOptions::LINEAR);
                    self.shown = Some((done.tag, texture));
                    self.error = None;
                }
                Err(message) => self.error = Some((done.tag, message)),
            }
        }
    }

    /// Whether the image of the requested tag has arrived (tests wait on it).
    pub fn is_ready(&self) -> bool {
        match (&self.requested, &self.shown) {
            (Some(r), Some((s, _))) => r == s,
            _ => false,
        }
    }
}
