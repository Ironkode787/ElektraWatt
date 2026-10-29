// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one dialog frame of the shell (WP-S2.17 part 4, §8 item 1; doc 16a §6.12): every modal dialog is drawn from
//! its [`DialogSpec`]. `color.surface.raised`, `elevation.2`, `radius.lg`, 440 px wide (small) or 640 px (forms),
//! `space.5` padding, over a backdrop of `color.backdrop` (slate-900 at 24 %); the title in `type.title`, left-aligned;
//! the body in `type.body`; the dialog's own content (fields, lists); the footer with the buttons right-aligned in the
//! platform's order (Windows: Primary then the others; macOS and Linux: the others then the Primary), the destructive
//! action left-aligned and apart. No close cross: Cancel is the way out; Escape cancels through `ui.dismiss`
//! (`App::route_keys`); the first focus is the Primary when the dialog has no field.

use egui::{Align, Id, Layout, RichText, Ui};
use ew_viewmodel::dialogs::{DialogAnswer, DialogButton, DialogSpec};

use crate::components::{self, Kind};
use crate::theme;

/// The answer of a click on one of the dialog's buttons.
pub(crate) type Clicked = Option<DialogAnswer>;

/// Paints the dialog of `spec` with `content` between the body and the footer. `focus_primary` gives the Primary the
/// keyboard focus (the frame the dialog opens, when it has no field). Returns the answer of a clicked button.
pub(crate) fn show(
    ctx: &egui::Context,
    spec: &DialogSpec,
    focus_primary: bool,
    content: impl FnOnce(&mut Ui),
) -> Clicked {
    let t = theme::current(ctx);
    let width = if spec.form {
        t.size_dialog_form
    } else {
        t.size_dialog_small
    };
    let frame = egui::Frame::new()
        .fill(t.surface_raised.color32())
        .stroke(theme::stroke(t.border_raised_width, t.border_default))
        .corner_radius(theme::radius(t.radius_lg))
        .shadow(theme::shadow(&t.elevation_2))
        .inner_margin(egui::Margin::same(t.space_5 as i8));
    let mut clicked = None;
    egui::Modal::new(Id::unique("ew-dialog"))
        .backdrop_color(t.backdrop.color32())
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width - 2.0 * t.space_5);
            ui.spacing_mut().item_spacing.y = t.space_3;
            let title = ui.label(
                RichText::new(&spec.title)
                    .font(theme::font(&t.type_title))
                    .color(t.text_primary.color32()),
            );
            let name = spec.title.clone();
            title.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
            for paragraph in &spec.body {
                ui.add(
                    egui::Label::new(
                        RichText::new(paragraph)
                            .font(theme::font(&t.type_body))
                            .color(t.text_primary.color32()),
                    )
                    .wrap(),
                );
            }
            content(ui);
            ui.add_space(t.space_2);
            clicked = footer(ui, spec, focus_primary);
        });
    clicked
}

/// The desktop platform whose footer order a dialog follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Platform {
    Windows,
    MacOs,
    Linux,
}

impl Platform {
    /// The platform the application runs on (anything but Windows and macOS follows the Linux order).
    pub(crate) fn host() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
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
/// Windows puts the Primary first, macOS and Linux put it last.
pub(crate) fn footer_order(platform: Platform, others: usize) -> Vec<Slot> {
    let rest = (0..others).map(Slot::Other);
    match platform {
        Platform::Windows => std::iter::once(Slot::Primary).chain(rest).collect(),
        Platform::MacOs | Platform::Linux => rest.chain(std::iter::once(Slot::Primary)).collect(),
    }
}

/// The footer: the destructive button at the left, the Primary and the others at the right in the platform's order.
fn footer(ui: &mut Ui, spec: &DialogSpec, focus_primary: bool) -> Clicked {
    let t = components::tokens(ui);
    let mut clicked = None;
    let mut press = |ui: &mut Ui, b: &DialogButton, kind: Kind, focus: bool| {
        let r = components::button(ui, kind, &b.label, true, t.size_button_min_width);
        if focus {
            r.request_focus();
        }
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
    fn windows_puts_the_primary_first() {
        assert_eq!(
            footer_order(Platform::Windows, 2),
            [Slot::Primary, Slot::Other(0), Slot::Other(1)]
        );
    }

    #[test]
    fn linux_puts_the_primary_last() {
        assert_eq!(
            footer_order(Platform::Linux, 2),
            [Slot::Other(0), Slot::Other(1), Slot::Primary]
        );
    }

    #[test]
    fn macos_puts_the_primary_last() {
        assert_eq!(footer_order(Platform::MacOs, 1), [Slot::Other(0), Slot::Primary]);
    }

    #[test]
    fn a_lone_primary_is_the_whole_group_everywhere() {
        for p in [Platform::Windows, Platform::MacOs, Platform::Linux] {
            assert_eq!(footer_order(p, 0), [Slot::Primary]);
        }
    }
}
