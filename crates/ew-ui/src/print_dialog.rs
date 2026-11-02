// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The print pre-dialog (WP-S3.07 part 3, D10 and ruling 4 of ID-1180; doc 16a §6.12): the options on the left on
//! §6.12's form rows, the preview of the print set on the right at the page's aspect ratio, "<n> of <m>" with
//! Previous and Next under it, the stamp visible. While the set builds, §6.19's progress line stands in the preview's
//! place; with no page chosen, §6.17's empty state. The dialog is `size.dialog.preview` wide, growing with the window
//! ([`crate::dialog::preview_width`]).
//!
//! **The preview** is the print set's page rendered by `ew-render-cpu`'s [`render_placed`] (D7's rule, as the PDF
//! writer draws it) at the resolution that fills the preview's pixels, on a thread of its own; the image is tagged
//! with the build's generation, the page, the resolution and the colour mode, and an image whose tag no longer
//! matches is dropped. On the frame clock (a visual run, the tests) the image is rendered in the frame that asks for
//! it.

use std::sync::{Arc, Mutex, PoisonError};

use egui::{Align, Layout, RichText, Sense, TextEdit, TextureHandle, TextureOptions, Ui, Vec2};
use ew_render_cpu::{PagePlacement, RenderOptions, RgbaImage, monochrome, render_placed};
use ew_viewmodel::ew_api::output::font_bytes;
use ew_viewmodel::ew_api::output::print::{Paper, PrintSet};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::print::{PrintBuild, PrintDialog, PrintRange};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind};
use crate::icons::IconId;
use crate::theme;

/// The share of the dialog's body the options column takes; the preview takes the rest.
const OPTIONS_SHARE: f32 = 0.36;
/// The preview's height at most, as a share of the window's height.
const PREVIEW_HEIGHT_SHARE: f32 = 0.56;
/// The preview's resolution bounds and step (dpi).
const DPI_MIN: u32 = 24;
const DPI_MAX: u32 = 200;
const DPI_STEP: u32 = 6;
/// The most rows the Sections list shows before it scrolls.
const SECTION_ROWS: f32 = 6.0;

/// What a preview image shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Tag {
    generation: u64,
    page: usize,
    dpi: u32,
    mono: bool,
}

type Slot = Arc<Mutex<Option<(Tag, Result<RgbaImage, String>)>>>;

/// The preview's image on show and the render in flight, kept in the context's memory.
#[derive(Clone, Default)]
struct Preview {
    shown: Option<(Tag, TextureHandle)>,
    asked: Option<Tag>,
    slot: Slot,
    error: Option<(Tag, String)>,
}

fn preview_id() -> egui::Id {
    egui::Id::unique("ew-print-preview")
}

/// The page at `index` of `set` placed on its paper: the content, monochrome when asked, and its placement (an
/// identity placement on the sheet for a page without one).
fn render_page(set: &PrintSet, index: usize, dpi: u32, mono: bool) -> Result<RgbaImage, String> {
    let page = set
        .pages
        .get(index)
        .ok_or_else(|| strings::text(key!("page-no-pages")).to_owned())?;
    let identity;
    let placement = match &page.placement {
        Some(p) => p,
        None => {
            identity = PagePlacement {
                width: page.width,
                height: page.height,
                scale: (1, 1),
                offset: ew_viewmodel::ew_api::output::ew_dl::ew_units::Pt2::ORIGIN,
                overlay: ew_viewmodel::ew_api::output::ew_dl::DisplayList::default(),
            };
            &identity
        }
    };
    let content = if mono {
        monochrome(&page.content)
    } else {
        page.content.clone()
    };
    let options = RenderOptions {
        dpi,
        ..RenderOptions::default()
    };
    render_placed(&content, placement, &options, font_bytes(), None)
        .map(|(image, _)| image)
        .map_err(|e| strings::format(key!("error-render"), &[&e.to_string()]))
}

/// The paper of the page at `index` (its placement's, else its sheet), in nm.
fn paper_of(set: &PrintSet, index: usize) -> Option<(f32, f32)> {
    let page = set.pages.get(index)?;
    let (w, h) = page
        .placement
        .as_ref()
        .map_or((page.width, page.height), |p| (p.width, p.height));
    Some((w.0 as f32, h.0 as f32))
}

/// The dpi at which a `paper_w_nm` wide paper fills `px` pixels: stepped, within bounds.
fn fit_dpi(paper_w_nm: f32, px: f32) -> u32 {
    let dpi = (px * 25_400_000.0 / paper_w_nm.max(1.0)).floor() as u32;
    (dpi / DPI_STEP * DPI_STEP).clamp(DPI_MIN, DPI_MAX)
}

/// Paints the pre-dialog's body: the options beside the preview. Its option changes, page moves and Enter (Print…,
/// once the set is built) go to `events`.
pub(crate) fn body(ui: &mut Ui, shell: &ShellModel, d: &PrintDialog, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let body = ui.available_width();
    let options_width = (body * OPTIONS_SHARE).floor();
    let preview_width = body - options_width - t.space_5;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_5;
        ui.allocate_ui_with_layout(
            Vec2::new(options_width, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(options_width);
                options(ui, d, just_opened, events);
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(preview_width, 0.0),
            Layout::top_down(Align::Center),
            |ui| {
                ui.set_width(preview_width);
                preview(ui, shell, d, events);
            },
        );
    });
    // Enter runs the Primary (doc 16a §6.12) once it can: Print….
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if enter && shell.print_set().is_some() && !egui::Popup::is_any_open(ui.ctx()) {
        events.push(Event::Action("print.send", json!({ "via": "printer" })));
    }
}

/// `print.set` with one option.
fn set(events: &mut Vec<Event>, params: Value) {
    events.push(Event::Action("print.set", params));
}

/// One form row: the label in the label column (`size.dialog.label`, then `space.3`), then `field`.
fn row<R>(ui: &mut Ui, label_width: f32, label: &str, field: impl FnOnce(&mut Ui, egui::Id) -> R) -> R {
    let t = theme::current(ui.ctx());
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                Vec2::new(label_width, t.size_control),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_size(Vec2::new(label_width, t.size_control));
                    ui.add(egui::Label::new(label).wrap())
                },
            )
            .inner;
        ui.add_space(t.space_3);
        ui.vertical(|ui| field(ui, label.id)).inner
    })
    .inner
}

/// The options column: Range, Paper, Scale to fit, Monochrome.
fn options(ui: &mut Ui, d: &PrintDialog, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    ui.spacing_mut().item_spacing.y = t.space_3;
    // The label column of §6.12, at most half the column (a narrower column keeps its fields).
    let label_width = t.size_dialog_label.min(ui.available_width() / 2.0).floor() - t.space_6;
    // The radio buttons are named by their own words; the row's label names the group.
    row(ui, label_width, strings::text(key!("print-range")), |ui, _| {
        ui.spacing_mut().item_spacing.y = t.space_2;
        for range in PrintRange::ALL {
            let r = ui.radio(d.range == range, strings::text(range.label_key()));
            if just_opened && d.range == range {
                r.request_focus();
            }
            if r.clicked() && d.range != range {
                set(events, json!({ "range": range.word() }));
            }
            match range {
                PrintRange::Sections => sections(ui, d, events),
                PrintRange::Pages => pages_field(ui, d, events),
                PrintRange::All | PrintRange::Current => {}
            }
        }
    });
    row(ui, label_width, strings::text(key!("print-paper")), |ui, label| {
        let combo = egui::ComboBox::from_id_salt("print-paper")
            .selected_text(PrintDialog::paper_text(d.paper))
            .width(ui.available_width())
            .icon(components::combo_chevron)
            .show_ui(ui, |ui| {
                for paper in Paper::ALL {
                    let words = PrintDialog::paper_text(paper);
                    if ui.selectable_label(d.paper == paper, words).clicked() && d.paper != paper {
                        set(events, json!({ "paper": paper.name() }));
                    }
                }
            });
        combo.response.labelled_by(label);
    });
    for (key, name, on) in [
        (key!("print-fit"), "fit", d.fit),
        (key!("print-mono"), "mono", d.monochrome),
    ] {
        row(ui, label_width, strings::text(key), |ui, label| {
            let mut value = on;
            let check = ui
                .add(egui::Checkbox::without_text(&mut value))
                .labelled_by(label);
            if check.changed() {
                let mut params = serde_json::Map::new();
                params.insert(name.to_owned(), Value::Bool(value));
                set(events, Value::Object(params));
            }
        });
    }
}

/// The Sections check list under its radio button, enabled under Sections.
fn sections(ui: &mut Ui, d: &PrintDialog, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let enabled = d.range == PrintRange::Sections;
    let mut list = |ui: &mut Ui| {
        for s in &d.sections {
            let mut ticked = s.ticked;
            let check = ui.add_enabled(enabled, egui::Checkbox::new(&mut ticked, s.label()));
            if check.changed() {
                let numbers: Vec<String> = d
                    .sections
                    .iter()
                    .filter(|o| if o.number == s.number { ticked } else { o.ticked })
                    .map(|o| o.number.to_string())
                    .collect();
                set(events, json!({ "sections": numbers.join(",") }));
            }
        }
    };
    ui.indent("print-sections", |ui| {
        // A short list shows whole; a long one scrolls after `SECTION_ROWS` rows.
        if d.sections.len() as f32 <= SECTION_ROWS {
            list(ui);
        } else {
            egui::ScrollArea::vertical()
                .id_salt("print-sections-list")
                .max_height(t.size_row * SECTION_ROWS)
                .show(ui, list);
        }
    });
}

/// The Pages field under its radio button, enabled under Pages, its refusal under it.
fn pages_field(ui: &mut Ui, d: &PrintDialog, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let enabled = d.range == PrintRange::Pages;
    ui.indent("print-pages", |ui| {
        let mut text = d.pages.clone();
        let edit = ui.add_enabled(
            enabled,
            TextEdit::singleline(&mut text)
                .desired_width(ui.available_width())
                .min_size(Vec2::new(0.0, t.size_control)),
        );
        let name = strings::text(key!("print-range-pages")).to_owned();
        edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, enabled, &name));
        if text != d.pages {
            set(events, json!({ "pages": text }));
        }
        if let Some(e) = &d.pages_error {
            components::live_line(ui, e, true);
        }
    });
}

/// The preview column: the page at its aspect ratio with its caption and Previous and Next, or the progress line, the
/// empty state or the failure in its place.
fn preview(ui: &mut Ui, shell: &ShellModel, d: &PrintDialog, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let width = ui.available_width();
    let max_height = (ui.ctx().content_rect().height() * PREVIEW_HEIGHT_SHARE).floor();
    // The box keeps the size of the page it shows (A4 landscape before anything is built), so the dialog does not
    // jump between states.
    let (paper_w, paper_h) = shell
        .print_set()
        .and_then(|s| paper_of(s, d.page))
        .unwrap_or((297.0, 210.0));
    let mut size = Vec2::new(width, width * paper_h / paper_w);
    if size.y > max_height {
        size = Vec2::new(max_height * paper_w / paper_h, max_height);
    }
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect.expand(1.0));
    painter.rect_filled(rect, theme::radius(t.radius_sm), t.surface_sunken.color32());
    match (&d.build, shell.print_set()) {
        (PrintBuild::Built { .. }, Some(set)) => {
            let ppp = ui.ctx().pixels_per_point();
            let tag = Tag {
                generation: d.generation,
                page: d.page,
                dpi: fit_dpi(paper_w, rect.width() * ppp),
                mono: d.monochrome,
            };
            if let Some(texture) = image(ui.ctx(), set, tag) {
                painter.image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    theme::NO_TINT,
                );
            }
            painter.rect_stroke(
                rect,
                theme::radius(t.radius_sm),
                theme::stroke(1.0, t.border_default),
                egui::StrokeKind::Outside,
            );
            navigation(ui, shell, d, set.pages.len(), events);
        }
        (PrintBuild::Building, _) => {
            let mut inner = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(t.space_5))
                    .layout(Layout::top_down(Align::Center)),
            );
            progress(&mut inner, shell);
            keep_navigation_room(ui);
        }
        (PrintBuild::Failed(cause), _) => {
            let mut inner = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(t.space_5))
                    .layout(Layout::top_down(Align::Center)),
            );
            components::live_line(&mut inner, &strings::format(key!("print-failed"), &[cause]), true);
            keep_navigation_room(ui);
        }
        _ => {
            let mut inner = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(t.space_5))
                    .layout(Layout::top_down(Align::Center)),
            );
            // Centred in the preview's place, as the progress line.
            let room = (inner.available_height() / 2.0 - 2.0 * t.size_control).max(0.0);
            inner.add_space(room);
            components::empty_state(
                &mut inner,
                IconId::printer,
                strings::text(key!("print-empty-title")),
                strings::text(key!("print-empty-text")),
                None,
            );
            keep_navigation_room(ui);
        }
    }
}

/// The room of the navigation row under a preview that shows none, so the dialog keeps its height between states.
fn keep_navigation_room(ui: &mut Ui) {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_2 + t.size_control);
}

/// §6.19's progress line in the preview's place: the text with its percentage over a 2-px accent line, or the
/// writing phase's text beside a spinner (none on the frame clock, which would never settle).
fn progress(ui: &mut Ui, shell: &ShellModel) {
    let t = theme::current(ui.ctx());
    let p = shell.print_progress().unwrap_or_default();
    let text = shell.print_progress_text().unwrap_or_default();
    ui.add_space((ui.available_height() / 2.0 - t.size_control).max(0.0));
    if p.total == 0 && !crate::clock::on_frame_clock() {
        ui.add(egui::Spinner::new().size(12.0));
    }
    components::live_line(ui, &text, false);
    if p.total > 0 {
        let width = (ui.available_width() * 0.6).floor();
        let (line, _) = ui.allocate_exact_size(Vec2::new(width, 2.0), Sense::hover());
        ui.painter().rect_filled(line, egui::CornerRadius::ZERO, t.border_default.color32());
        let done = egui::Rect::from_min_size(
            line.min,
            Vec2::new(line.width() * p.done as f32 / p.total as f32, 2.0),
        );
        ui.painter().rect_filled(done, egui::CornerRadius::ZERO, t.accent.color32());
    }
}

/// "<n> of <m>" with Previous and Next under the preview.
fn navigation(ui: &mut Ui, shell: &ShellModel, d: &PrintDialog, pages: usize, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        let previous = strings::text(key!("print-previous"));
        let next = strings::text(key!("print-next"));
        let caption = shell.print_page_text().unwrap_or_default();
        // Centred: the row's own width is the two buttons and the caption.
        let caption_width = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                caption.clone(),
                theme::font(&t.type_body),
                theme::NONE,
            )
            .size()
            .x
        });
        let buttons = 2.0 * t.size_button_min_width.max(0.0) + caption_width + 2.0 * t.space_3;
        ui.add_space(((ui.available_width() - buttons) / 2.0).max(0.0));
        if components::button(ui, Kind::Secondary, previous, d.page > 0, t.size_button_min_width).clicked() {
            events.push(Event::Action("print.page", json!({ "to": "previous" })));
        }
        ui.label(
            RichText::new(&caption)
                .font(theme::font(&t.type_body))
                .color(t.text_secondary.color32()),
        );
        if components::button(ui, Kind::Secondary, next, d.page + 1 < pages, t.size_button_min_width).clicked() {
            events.push(Event::Action("print.page", json!({ "to": "next" })));
        }
    });
}

/// The texture of `tag`, rendering it when it is not there yet (on its own thread, or now on the frame clock).
fn image(ctx: &egui::Context, set: &Arc<PrintSet>, tag: Tag) -> Option<TextureHandle> {
    let mut state: Preview = ctx.data_mut(|m| m.get_temp(preview_id())).unwrap_or_default();
    let missing = state.shown.as_ref().is_none_or(|(s, _)| *s != tag);
    if missing && state.asked != Some(tag) && state.error.as_ref().is_none_or(|(e, _)| *e != tag) {
        state.asked = Some(tag);
        let slot = Arc::clone(&state.slot);
        let set = Arc::clone(set);
        let render = move || {
            let result = render_page(&set, tag.page, tag.dpi, tag.mono);
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some((tag, result));
        };
        if crate::clock::on_frame_clock() {
            // In the frame that asks for it.
            render();
        } else {
            let waker = ctx.clone();
            let spawned = std::thread::Builder::new()
                .name("ew-print-preview".to_owned())
                .spawn(move || {
                    render();
                    waker.request_repaint();
                });
            if spawned.is_err() {
                state.error = Some((tag, strings::text(key!("error-no-thread")).to_owned()));
            }
        }
    }
    // A finished render: the asked one is uploaded, an older one dropped.
    let done = state.slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some((done_tag, result)) = done {
        match result {
            Ok(image) if done_tag == tag => {
                let size = [image.width as usize, image.height as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, &image.pixels);
                let texture = ctx.load_texture("print-preview", color, TextureOptions::LINEAR);
                state.shown = Some((done_tag, texture));
                state.error = None;
            }
            Err(message) if done_tag == tag => state.error = Some((done_tag, message)),
            // An older render may have taken the slot of the asked one: it is asked again.
            _ => state.asked = None,
        }
    }
    let shown = state
        .shown
        .as_ref()
        .filter(|(s, _)| *s == tag)
        .map(|(_, tex)| tex.clone());
    // The image of the same page at another resolution stays on show until the new one is in.
    let fallback = state
        .shown
        .as_ref()
        .filter(|(s, _)| s.generation == tag.generation && s.page == tag.page && s.mono == tag.mono)
        .map(|(_, tex)| tex.clone());
    ctx.data_mut(|m| m.insert_temp(preview_id(), state));
    shown.or(fallback)
}
