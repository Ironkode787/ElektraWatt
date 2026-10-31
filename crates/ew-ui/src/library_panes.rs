// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11, FR-MAC-006, FT-487): what the Macros and Libraries navigators show besides their rows.
//!
//! **The Libraries navigator** has an Update… button (Secondary) above its rows: `libraries.update` previews
//! `library.update` of the marked copies, else the copy under the cursor, else every copy with an update, and opens
//! the bulk card in the Preview tab. The rows' copy state is the API's column and badge, in words.
//!
//! **The Macros navigator** has a detail block under its rows for the macro under the cursor: its name
//! (`type.body.strong`), version and library, description, the thumbnail (FT-754 (a): the macro drawn on a blank page
//! of a throwaway copy, `ShellModel::macro_thumbnail`, rendered by the CPU renderer once per macro version and shown
//! on the white sheet with a 1-px `color.border.default` frame; "No preview" when there is none), the devices, the
//! variables and Insert macro… (Primary), which arms the macro tool. The thumbnail is an `Image` named "Preview of
//! the macro".
//!
//! **Accessibility.** The detail block is a `Group` named after the macro; each line is a label; the buttons are named
//! by their text (NFR-A11Y-01).

use egui::{RichText, Ui};
use ew_viewmodel::ew_api::output::MacroThumbnail;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::theme;

/// Paints the parts of navigator `kind` that come before its rows (the Libraries navigator's Update…), and the
/// Macros navigator's detail block as a bottom panel; nothing for another kind.
pub(crate) fn show(ui: &mut Ui, shell: &ShellModel, kind: NavKind, events: &mut Vec<Event>) {
    match kind {
        NavKind::Libraries => {
            update_bar(ui, shell, events);
            if let Some(detail) = shell.library_copy_detail() {
                let t = components::tokens(ui);
                let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(
                    crate::theme::margin_px(t.space_4),
                    crate::theme::margin_px(t.space_2),
                ));
                egui::Panel::bottom(ui.scope_id().with("copy-detail"))
                    .resizable(false)
                    .frame(frame)
                    .show(ui, |ui| copy_detail(ui, detail));
            }
        }
        NavKind::Macros => {
            if let Some(detail) = shell.macro_detail() {
                let room = ui.available_height();
                let t = components::tokens(ui);
                let frame = egui::Frame::new().inner_margin(egui::Margin::symmetric(
                    crate::theme::margin_px(t.space_4),
                    crate::theme::margin_px(t.space_2),
                ));
                egui::Panel::bottom(ui.scope_id().with("macro-detail"))
                    .resizable(false)
                    .frame(frame)
                    .show(ui, |ui| {
                        macro_detail(ui, detail, shell.macro_thumbnail(), room, events)
                    });
            }
        }
        _ => {}
    }
}

/// The Libraries navigator's Update… button.
fn update_bar(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let enabled = ew_viewmodel::actions::find("libraries.update")
        .is_some_and(|m| ew_viewmodel::actions::enabled_with(m, shell, &serde_json::Map::new()).is_ok());
    ui.horizontal(|ui| {
        ui.add_space(t.space_4);
        if components::button(
            ui,
            Kind::Secondary,
            strings::text("action-libraries-update"),
            enabled,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("libraries.update", json!({})));
        }
    });
    ui.add_space(t.space_2);
}

/// The detail block of a project's library copy (FT-755): its name (`type.body.strong`), its kind and key, its version
/// and library, its state; a `Group` named after the copy.
fn copy_detail(ui: &mut Ui, d: &ew_viewmodel::ew_api::output::LibraryCopyDetail) {
    let t = components::tokens(ui);
    let id = ui.scope_id().with("copy-detail-group");
    let rect = ui.available_rect_before_wrap();
    let group = ui.interact(rect, id, egui::Sense::hover());
    let name = d.name.clone();
    group.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(id), |ui| {
        ui.spacing_mut().item_spacing.y = t.space_1;
        let line = |ui: &mut Ui, text: &str, font: &theme::TypeToken, colour: theme::Rgba| {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(theme::font(font))
                        .color(colour.color32()),
                )
                .wrap(),
            );
        };
        line(ui, &d.name, &t.type_body_strong, t.text_primary);
        let kind = if d.key.is_empty() {
            d.kind.to_owned()
        } else {
            strings::format("library-copy-kind-key", &[d.kind, &d.key])
        };
        line(ui, &kind, &t.type_small, t.text_secondary);
        let library = if d.library.is_empty() {
            strings::text("library-copy-not-installed")
        } else {
            &d.library
        };
        line(
            ui,
            &strings::format("macro-version", &[&d.version, library]),
            &t.type_small,
            t.text_secondary,
        );
        line(ui, &d.state, &t.type_small, t.text_primary);
    });
}

/// The thumbnail box's height in points; its width is the detail block's.
const THUMBNAIL_HEIGHT: f32 = 64.0;

/// The thumbnail's height in a detail region `cap` high whose lists take `lists` (W40a 5f): [`THUMBNAIL_HEIGHT`]
/// when both fit, else what the lists leave, never under `floor` (the lists then scroll).
fn thumbnail_height(cap: f32, lists: f32, floor: f32) -> f32 {
    (cap - lists).clamp(floor.min(THUMBNAIL_HEIGHT), THUMBNAIL_HEIGHT)
}

#[cfg(test)]
mod thumbnail_tests {
    /// W40a 5f: the thumbnail keeps its height where the lists fit, gives up height at the floor down to the floor.
    #[test]
    fn the_thumbnail_yields_to_the_lists() {
        use super::{THUMBNAIL_HEIGHT, thumbnail_height};
        assert_eq!(thumbnail_height(400.0, 100.0, 28.0), THUMBNAIL_HEIGHT);
        assert_eq!(thumbnail_height(150.0, 110.0, 28.0), 40.0);
        assert_eq!(thumbnail_height(150.0, 300.0, 28.0), 28.0);
    }
}

/// Paints a macro's thumbnail (FT-754 (a)) in a box of the block's width and [`THUMBNAIL_HEIGHT`]: the region of the
/// sheet the macro covers, rendered at the dpi that fits the box in physical pixels (12 to 300), centred on white
/// with a 1-px `color.border.default` frame. The texture of the macro on show is kept in the context's memory (by copy,
/// version, dpi and region).
/// `false` when the render fails (the caller then says "No preview").
fn thumbnail_ui(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::output::MacroDetail,
    th: &MacroThumbnail,
    height: f32,
) -> bool {
    let t = components::tokens(ui);
    let width = ui.available_width().max(1.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let name = strings::format("macro-preview-name", &[&d.name]);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &name));
    let [x0, y0, x1, y1] = th.region;
    let (rw, rh) = ((x1 - x0).max(1) as f64, (y1 - y0).max(1) as f64);
    let ppp = f64::from(ui.ctx().pixels_per_point());
    let inner = (f64::from(width) - 8.0).max(1.0) * ppp;
    let inner_h = (f64::from(height) - 8.0).max(1.0) * ppp;
    // pixels per nm at 1 dpi: 1 / 25.4e6
    let dpi = (inner / rw).min(inner_h / rh) * 25_400_000.0;
    let dpi = (dpi.floor() as u32).clamp(12, 300);
    let key = egui::Id::unique(("macro-thumbnail", d.copy, d.version.as_str(), dpi, th.region));
    // One slot: the texture of the macro on show (another macro, version or size replaces it).
    let slot = egui::Id::unique("macro-thumbnail-slot");
    let kept = ui
        .ctx()
        .data(|m| m.get_temp::<(egui::Id, Option<egui::TextureHandle>)>(slot))
        .filter(|(k, _)| *k == key)
        .map(|(_, t)| t);
    let texture = match kept {
        Some(t) => t,
        None => {
            let made = render_thumbnail(th, dpi).map(|image| {
                let colour = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.pixels,
                );
                ui.ctx()
                    .load_texture("macro-thumbnail", colour, egui::TextureOptions::LINEAR)
            });
            ui.ctx().data_mut(|m| m.insert_temp(slot, (key, made.clone())));
            made
        }
    };
    let Some(texture) = texture else {
        return false;
    };
    let size = texture.size_vec2() / ui.ctx().pixels_per_point();
    let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
    let shown = egui::Rect::from_center_size(rect.center(), size * scale);
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_sm), t.canvas_sheet.color32());
    painter.image(
        texture.id(),
        shown,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        theme::NO_TINT,
    );
    painter.rect_stroke(
        rect,
        theme::radius(t.radius_sm),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    true
}

/// The region of a thumbnail rendered at `dpi` by the CPU renderer with the bundled fonts; `None` on a render error.
fn render_thumbnail(th: &MacroThumbnail, dpi: u32) -> Option<ew_render_cpu::RgbaImage> {
    let [x0, y0, x1, y1] = th.region;
    let px = |nm: i64| (i128::from(nm) * i128::from(dpi) / 25_400_000) as i64;
    let (x, y) = (px(x0), px(y0));
    let width = u32::try_from(px(x1) - x).ok()?.clamp(1, 4096);
    let height = u32::try_from(px(y1) - y).ok()?.clamp(1, 4096);
    let options = ew_render_cpu::RegionOptions {
        dpi,
        x,
        y,
        width,
        height,
        background: ew_viewmodel::ew_api::output::ew_dl::Color::WHITE,
        surround: ew_viewmodel::ew_api::output::ew_dl::Color::WHITE,
    };
    let sheet = ew_render_cpu::SheetSize {
        width: ew_viewmodel::ew_api::output::ew_dl::ew_units::Nm(th.sheet.0),
        height: ew_viewmodel::ew_api::output::ew_dl::ew_units::Nm(th.sheet.1),
    };
    let bounds = ew_render_cpu::ItemBounds::of(&th.content, None);
    ew_render_cpu::render_region(
        &th.content,
        Some(&bounds),
        sheet,
        &options,
        ew_viewmodel::ew_api::output::font_bytes(),
        None,
    )
    .ok()
    .map(|(image, _)| image)
}

/// The detail block of a macro.
fn macro_detail(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::output::MacroDetail,
    thumbnail: Option<&MacroThumbnail>,
    room: f32,
    events: &mut Vec<Event>,
) {
    let t = components::tokens(ui);
    let id = ui.scope_id().with("macro-detail-group");
    let rect = ui.available_rect_before_wrap();
    let group = ui.interact(rect, id, egui::Sense::hover());
    let name = d.name.clone();
    group.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(id), |ui| {
        ui.spacing_mut().item_spacing.y = t.space_1;
        let line = |ui: &mut Ui, text: &str, font: &theme::TypeToken, colour: theme::Rgba| {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(theme::font(font))
                        .color(colour.color32()),
                )
                .wrap(),
            );
        };
        // Cycle 39 (FIX39b-1): the key follows the name as secondary text (the Libraries navigator's copy detail
        // shows name, kind and key); on the name's row, so the block keeps its height and its Insert button.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_2;
            line(ui, &d.name, &t.type_body_strong, t.text_primary);
            if !d.key.is_empty() {
                line(ui, &d.key, &t.type_small, t.text_secondary);
            }
        });
        line(
            ui,
            &strings::format("macro-version", &[&d.version, &d.library]),
            &t.type_small,
            t.text_secondary,
        );
        if !d.description.is_empty() {
            line(ui, &d.description, &t.type_small, t.text_primary);
        }
        // Cycle 39 (FIX39b-1): the thumbnail and the lists scroll within half the navigator's room, so a long macro
        // never pushes the Insert button out of the pane (at any window size; the room is the pane's). W40a 5f: the
        // thumbnail gives up height first (down to a token's worth), so the lists' lines show whole at the floor;
        // a list longer than the region scrolls with its bar on show.
        let cap = (room / 2.0).max(4.0 * t.size_control);
        let line_h = ui.fonts_mut(|f| f.row_height(&theme::font(&t.type_small))) + t.space_1;
        let lines: usize = [&d.devices, &d.variables]
            .iter()
            .filter(|items| !items.is_empty())
            .map(|items| items.len() + 1)
            .sum();
        let thumb = thumbnail_height(cap, lines as f32 * line_h + t.space_1, t.size_control);
        egui::ScrollArea::vertical()
            .id_salt("macro-detail-lists")
            .max_height(cap)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                // §11: the thumbnail, or "No preview".
                if !thumbnail.is_some_and(|th| thumbnail_ui(ui, d, th, thumb)) {
                    line(
                        ui,
                        strings::text("macro-no-preview"),
                        &t.type_small,
                        t.text_tertiary,
                    );
                }
                for (title, items) in [
                    (strings::text("macro-devices"), &d.devices),
                    (strings::text("macro-variables"), &d.variables),
                ] {
                    if items.is_empty() {
                        continue;
                    }
                    line(
                        ui,
                        &format!("{title} ({})", items.len()),
                        &t.type_small,
                        t.text_secondary,
                    );
                    for item in items {
                        line(ui, item, &t.type_small, t.text_primary);
                    }
                }
            });
        if d.reference.is_some() {
            ui.add_space(t.space_1);
            if components::button(
                ui,
                Kind::Primary,
                strings::text("action-macros-insert"),
                true,
                0.0,
            )
            .clicked()
            {
                events.push(Event::Action("macros.insert", json!({})));
            }
        }
    });
}
