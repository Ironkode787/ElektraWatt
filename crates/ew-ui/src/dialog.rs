// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one dialog frame of the shell (doc 16a §6.12): every modal dialog is drawn from
//! its [`DialogSpec`]. `color.surface.raised`, `elevation.2`, `radius.lg`, 440 px wide (small) or 640 px (forms, growing with the window;
//! the print pre-dialog 1040 px, growing too),
//! `space.5` padding, over a backdrop of `color.backdrop` (slate-900 at 24 %); the title in `type.title`, left-aligned;
//! the body in `type.body`; the dialog's own content (fields, lists); the footer with the buttons right-aligned in the
//! platform's order (Windows: Primary then the others; Linux: the others then the Primary), the destructive
//! action left-aligned and apart. No close cross: Cancel is the way out; Escape cancels through `ui.dismiss`
//! (`App::route_keys`); the first focus is the Primary when the dialog has no field.

use egui::{Align, Id, Layout, Ui};
use ew_viewmodel::dialogs::{DialogAnswer, DialogButton, DialogSpec};

use crate::components::{self, Kind, Tip};
use crate::theme;
use crate::theme::shares::{FORM_SHARE, PREVIEW_SHARE, SMALL_SHARE};

/// The width of the main resolution, 1920 × 1080 points (doc 16a §2): the window every default layout is designed
/// for.
pub(crate) const MAIN_WIDTH: f32 = 1920.0;

/// A form dialog's width in a window `window` wide: `size.dialog.form`, growing with the window ([`FORM_SHARE`]).
pub(crate) fn form_width(t: &crate::theme::Tokens, window: f32) -> f32 {
    grown(t.size_dialog_form, FORM_SHARE, window)
}

/// The width of a creation form of a few fields (New page…, Terminal…): [`form_width`] up to its width at the main
/// resolution (864), since its short values gain nothing from wider fields; the forms that hold a list grow on.
pub(crate) fn fields_form_width(t: &crate::theme::Tokens, window: f32) -> f32 {
    form_width(t, window.min(MAIN_WIDTH))
}

/// The width of a small dialog that holds a list in a window `window` wide: `size.dialog.small`, growing with the
/// window ([`SMALL_SHARE`]).
pub(crate) fn small_width(t: &crate::theme::Tokens, window: f32) -> f32 {
    grown(t.size_dialog_small, SMALL_SHARE, window)
}

/// The print pre-dialog's width in a window `window` wide: `size.dialog.preview`, growing with the window
/// ([`PREVIEW_SHARE`]).
pub(crate) fn preview_width(t: &crate::theme::Tokens, window: f32) -> f32 {
    grown(t.size_dialog_preview, PREVIEW_SHARE, window)
}

/// A width that grows with the window: `share` of `window`, never under `base` (its token) and never wider than the
/// window.
pub(crate) fn grown(base: f32, share: f32, window: f32) -> f32 {
    (window * share).max(base).min(window)
}

/// The answer of a click on one of the dialog's buttons.
pub(crate) type Clicked = Option<DialogAnswer>;

/// Paints the dialog of `spec` with `content` between the body and the footer, at `width` when given (the print
/// pre-dialog's [`preview_width`]), else at the width of the spec's kind. `focus_primary` gives the Primary the
/// keyboard focus (the frame the dialog opens, when it has no field). `refusal`: why the button pressed last did
/// nothing, in `color.error` over the buttons, as a form's refusal stands. Returns the answer of a clicked button.
pub(crate) fn show_sized(
    ctx: &egui::Context,
    spec: &DialogSpec,
    width: Option<f32>,
    focus_primary: bool,
    refusal: Option<&str>,
    content: impl FnOnce(&mut Ui),
) -> Clicked {
    let t = theme::current(ctx);
    // A form grows with the window (doc 16a §2), its fields and lists take the gain.
    let width = match width {
        Some(w) => w,
        None if spec.form => form_width(&t, ctx.content_rect().width()),
        // A small dialog is as wide as its buttons need (FT-1736: four buttons ran out past its left edge, so the
        // title stood indented past them), never under `size.dialog.small` nor wider than the window.
        None => (footer_width(ctx, &t, spec) + 2.0 * t.space_5)
            .max(t.size_dialog_small)
            .min(ctx.content_rect().width()),
    };
    let frame = crate::components::raised_frame(&t, t.space_5);
    let mut clicked = None;
    egui::Modal::new(Id::unique("ew-dialog"))
        .backdrop_color(t.backdrop.color32())
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width - 2.0 * t.space_5);
            ui.spacing_mut().item_spacing.y = t.space_3;
            let title = ui.label(crate::components::title(&t, &spec.title));
            let name = spec.title.clone();
            title.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
            for paragraph in &spec.body {
                ui.add(egui::Label::new(crate::components::body(&t, paragraph)).wrap());
            }
            content(ui);
            if let Some(text) = refusal {
                components::live_line(ui, text, true);
            }
            ui.add_space(t.space_2);
            // The footer's own scope, so its buttons keep their ids (and the focus) when the body above
            // gains a line (a form's refusal).
            clicked = ui
                .scope_builder(
                    egui::UiBuilder::new().scope_id(Id::unique("ew-dialog-footer")),
                    |ui| footer(ui, spec, focus_primary),
                )
                .inner;
        });
    clicked
}

/// The desktop platform whose footer order a dialog follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Platform {
    Windows,
    Linux,
}

impl Platform {
    /// The platform the application runs on (anything but Windows follows the Linux order).
    pub(crate) fn host() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// One slot of the footer's right-hand group: the Primary, or the n-th other button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Primary,
    Other(usize),
}

/// The reading order (left to right) of the footer's right-hand group on `platform` with `others` other buttons:
/// Windows puts the Primary first, Linux puts it last.
pub(crate) fn footer_order(platform: Platform, others: usize) -> Vec<Slot> {
    let rest = (0..others).map(Slot::Other);
    match platform {
        Platform::Windows => std::iter::once(Slot::Primary).chain(rest).collect(),
        Platform::Linux => rest.chain(std::iter::once(Slot::Primary)).collect(),
    }
}

/// The width the footer's buttons take side by side: each its label in its font plus the button padding, at least
/// `size.button.min-width`, and `space.3` between them.
fn footer_width(ctx: &egui::Context, t: &crate::theme::Tokens, spec: &DialogSpec) -> f32 {
    let pad = 2.0 * ctx.global_style().spacing.button_padding.x;
    let width = |b: &DialogButton, font: &crate::theme::TypeToken| {
        let galley = ctx.fonts_mut(|f| f.layout_no_wrap(b.label.clone(), theme::font(font), theme::NONE));
        (galley.size().x + pad).max(t.size_button_min_width)
    };
    let buttons: Vec<f32> = std::iter::once(width(&spec.primary, &t.type_body_strong))
        .chain(spec.secondary.iter().map(|b| width(b, &t.type_body)))
        .chain(spec.danger.iter().map(|b| width(b, &t.type_body)))
        .collect();
    buttons.iter().sum::<f32>() + t.space_3 * buttons.len().saturating_sub(1) as f32
}

/// The footer: the destructive button at the left, the Primary and the others at the right in the platform's order.
fn footer(ui: &mut Ui, spec: &DialogSpec, focus_primary: bool) -> Clicked {
    let t = components::tokens(ui);
    let mut clicked = None;
    let mut press = |ui: &mut Ui, b: &DialogButton, kind: Kind, focus: bool| {
        // A disabled button says why on hover (Create enclosure's Create without a location or a part).
        let r = components::button(ui, kind, &b.label, b.disabled.is_none(), t.size_button_min_width);
        let r = match &b.disabled {
            Some(reason) => r.disabled_tip(reason.as_str()),
            None => r,
        };
        if focus && b.disabled.is_none() {
            r.request_focus();
        }
        // The focused button shows its focus ring (doc 16a §6.1).
        components::focus_ring(ui, &r, t.radius_md);
        if r.clicked() {
            clicked = Some(b.answer.clone());
        }
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        if let Some(d) = &spec.danger {
            let kind = if spec.confirms_destruction() {
                Kind::DangerPrimary
            } else {
                Kind::Danger
            };
            press(ui, d, kind, false);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // In a confirmation of a destructive step the Primary (Cancel) is drawn as a Secondary beside the red step.
            let primary_kind = if spec.confirms_destruction() {
                Kind::Secondary
            } else {
                Kind::Primary
            };
            // Right to left: the last button of the reading order first.
            let primary = (&spec.primary, primary_kind, focus_primary);
            let others = spec.secondary.iter().map(|b| (b, Kind::Secondary, false));
            let mut order: Vec<(&DialogButton, Kind, bool)> = Vec::new();
            let mut others = others;
            for slot in footer_order(Platform::host(), spec.secondary.len()) {
                match slot {
                    Slot::Primary => order.push(primary),
                    Slot::Other(_) => order.extend(others.next()),
                }
            }
            order.reverse();
            for (b, kind, focus) in order {
                press(ui, b, kind, focus);
            }
        });
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::{Platform, Slot, footer_order};

    #[test]
    fn windows_puts_the_primary_first_linux_last_and_a_lone_primary_is_the_whole_group() {
        assert_eq!(
            footer_order(Platform::Windows, 2),
            [Slot::Primary, Slot::Other(0), Slot::Other(1)],
            "windows"
        );
        assert_eq!(
            footer_order(Platform::Linux, 2),
            [Slot::Other(0), Slot::Other(1), Slot::Primary],
            "linux"
        );
        for p in [Platform::Windows, Platform::Linux] {
            assert_eq!(footer_order(p, 0), [Slot::Primary], "{p:?} alone");
        }
    }

    /// A form, a small list dialog and the print pre-dialog keep their token width in a small window, are wider at
    /// 2560 than at 1920 and stay inside a window narrower than their token; a form of fields stops at its 1920 width.
    #[test]
    fn grown_dialogs_are_wider_at_2560_than_at_1920() {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        for (name, f, base) in [
            (
                "form",
                super::form_width as fn(&crate::theme::Tokens, f32) -> f32,
                t.size_dialog_form,
            ),
            ("small", super::small_width, t.size_dialog_small),
            ("preview", super::preview_width, t.size_dialog_preview),
        ] {
            let (main, extension) = (f(&t, 1920.0), f(&t, 2560.0));
            assert_eq!(f(&t, 1024.0).max(base), base, "{name} in a small window");
            assert!(
                extension > main && main > base,
                "{name}: {main} at 1920, {extension} at 2560"
            );
            assert_eq!(f(&t, 400.0), 400.0, "{name} within a narrow window");
        }
        assert_eq!(
            super::fields_form_width(&t, 2560.0),
            super::form_width(&t, 1920.0)
        );
        assert_eq!(super::fields_form_width(&t, 400.0), 400.0);
    }
}
