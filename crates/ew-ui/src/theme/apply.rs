// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The mapping of the tokens onto egui (doc 16a §4.14): [`style_of`] builds the `Style` of one theme and density,
//! [`apply`] installs it in its egui slot, [`apply_setting`] installs both slots of a `theme` setting.

use std::sync::Arc;

use egui::{FontFamily, FontId, Margin, Style, TextStyle, Vec2, Visuals};

use super::{Density, Tokens, TypeToken, install, radius, shadow, stroke, themes_of_setting, tokens};
use crate::fonts::SEMIBOLD_FAMILY;

/// The font of a type token: Noto Sans Regular, or the SemiBold family (D8).
pub fn font(t: &TypeToken) -> FontId {
    if t.semibold {
        FontId::new(t.size, FontFamily::Name(SEMIBOLD_FAMILY.into()))
    } else {
        FontId::proportional(t.size)
    }
}

/// The egui `Style` of `t`, from egui's default style of the theme's slot (doc 16a §4.14 sets the rest).
pub fn style_of(t: &Tokens) -> Style {
    let mut style = Style {
        visuals: if t.theme.is_dark() {
            Visuals::dark()
        } else {
            Visuals::light()
        },
        ..Style::default()
    };
    let field = t.surface_panel;
    let hover_fill = t.state_hover.over(field);
    let pressed_fill = t.state_pressed.over(field);
    let v = &mut style.visuals;
    v.dark_mode = t.theme.is_dark();
    v.panel_fill = t.surface_panel.color32();
    v.window_fill = t.surface_raised.color32();
    v.window_stroke = stroke(t.border_raised_width, t.border_default);
    v.window_shadow = shadow(&t.elevation_2);
    v.window_corner_radius = radius(t.radius_lg);
    v.popup_shadow = shadow(&t.elevation_1);
    v.menu_corner_radius = radius(t.radius_md);
    v.extreme_bg_color = field.color32();
    v.text_edit_bg_color = Some(field.color32());
    v.faint_bg_color = t.surface_sunken.color32();
    v.code_bg_color = t.surface_sunken.color32();
    v.selection.bg_fill = t.accent_subtle.color32();
    v.selection.stroke = stroke(t.border_width, t.accent);
    v.hyperlink_color = t.accent.color32();
    v.warn_fg_color = t.warning.color32();
    v.error_fg_color = t.error.color32();
    v.weak_text_color = Some(t.text_tertiary.color32());
    v.override_text_color = None;
    v.striped = false;
    v.indent_has_left_vline = false;
    v.button_frame = true;
    v.collapsing_header_frame = false;
    // A 1-px caret (`border.width`, as the platforms draw it), so it clears the first glyph of the text or
    // the placeholder; the 2-px caret, centred on the pen position, overlapped it.
    v.text_cursor.stroke = stroke(t.border_width, t.accent);

    let corner = radius(t.radius_md);
    let w = &mut v.widgets;
    // Labels and other non-interactive text read as body text (the mockup's `body` colour).
    w.noninteractive.bg_fill = t.surface_panel.color32();
    w.noninteractive.weak_bg_fill = t.surface_panel.color32();
    w.noninteractive.bg_stroke = stroke(t.border_width, t.border_subtle);
    w.noninteractive.fg_stroke = stroke(t.border_width, t.text_primary);
    w.noninteractive.corner_radius = corner;
    w.noninteractive.expansion = 0.0;
    w.inactive.weak_bg_fill = super::NONE;
    w.inactive.bg_fill = field.color32();
    w.inactive.bg_stroke = stroke(t.border_width, t.border_control);
    w.inactive.fg_stroke = stroke(t.border_width, t.text_primary);
    w.inactive.corner_radius = corner;
    w.inactive.expansion = 0.0;
    for (state, fill) in [
        (&mut w.hovered, hover_fill),
        (&mut w.open, hover_fill),
        (&mut w.active, pressed_fill),
    ] {
        state.weak_bg_fill = fill.color32();
        state.bg_fill = fill.color32();
        state.bg_stroke = stroke(t.border_width, t.border_control);
        state.fg_stroke = stroke(t.border_width, t.text_primary);
        state.corner_radius = corner;
        state.expansion = 0.0;
    }

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(t.space_3, t.space_2);
    s.button_padding = Vec2::new(t.space_4, t.space_2);
    s.interact_size.y = t.size_control;
    s.indent = t.size_tree_indent;
    s.menu_margin = Margin::same(crate::theme::margin_px(t.space_2));
    s.window_margin = Margin::same(crate::theme::margin_px(t.space_5));
    s.icon_width = t.icon_sm;
    s.icon_spacing = t.space_icon;
    s.tooltip_width = t.size_tooltip_max;

    style.text_styles = [
        (TextStyle::Small, font(&t.type_caption)),
        (TextStyle::Body, font(&t.type_body)),
        (TextStyle::Button, font(&t.type_body)),
        (TextStyle::Heading, font(&t.type_title)),
        (TextStyle::Monospace, FontId::monospace(t.type_mono.size)),
        (TextStyle::Name("small".into()), font(&t.type_small)),
        (TextStyle::Name("strong".into()), font(&t.type_body_strong)),
        (TextStyle::Name("display".into()), font(&t.type_display)),
    ]
    .into();
    style.animation_time = t.motion_fast as f32 / 1000.0;
    style
}

/// With reduced motion a scroll area moves by the wheel in the frame the wheel turns.
/// egui 0.37 smooths a wheel's line and page steps over the next frames and has no switch for it; its pass's scroll
/// delta is public input, so at the start of each frame it is set to this frame's own wheel events, in points as egui
/// reads them (a line `line_scroll_speed`, a page the viewport's height, a horizontal or vertical modifier as egui
/// applies it, the zoom modifier's turns left to the zoom). The steps egui still holds for later frames are then
/// overwritten as they come out, so no motion follows the turn.
pub fn unsmoothed_scroll(ctx: &egui::Context) {
    let o = ctx.options(|o| o.input_options);
    ctx.input_mut(|i| {
        let page = i.viewport_rect().height();
        let mut sum = Vec2::ZERO;
        for e in &i.events {
            let egui::Event::MouseWheel {
                unit,
                delta,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            if modifiers.matches_any(o.zoom_modifier) {
                continue;
            }
            let mut d = match unit {
                egui::MouseWheelUnit::Point => *delta,
                egui::MouseWheelUnit::Line => o.line_scroll_speed * *delta,
                egui::MouseWheelUnit::Page => page * *delta,
            };
            let (horizontal, vertical) = (
                modifiers.matches_any(o.horizontal_scroll_modifier),
                modifiers.matches_any(o.vertical_scroll_modifier),
            );
            if horizontal && !vertical {
                d = Vec2::new(d.x + d.y, 0.0);
            } else if vertical && !horizontal {
                d = Vec2::new(0.0, d.x + d.y);
            }
            sum += d;
        }
        i.smooth_scroll_delta = sum;
    });
}

/// Installs `tokens` in `ctx`: the style of its egui slot (light or dark) and the theme preference for that slot, so
/// that `ctx.style()` is this style from the next pass.
pub fn apply(tokens: &Tokens, ctx: &egui::Context) {
    ctx.set_style_of(tokens.theme.egui(), Arc::new(style_of(tokens)));
    install(ctx, tokens);
    ctx.set_theme(tokens.theme.egui());
}

/// Installs the `theme` and `density` settings (`settings.toml`): both egui slots, then the preference, so that
/// `system` follows the operating system (D5). The settings `min_font_pt` and
/// `reduce_motion` (as `ShellModel::reduce_motion` resolves it) come as two more arguments; `min_font_pt` raises every
/// text token below it to it, and reduced motion sets the motion tokens, egui's animation time with them,
/// to zero and scrolls to a target without animation.
pub fn apply_setting(theme: &str, density: &str, min_font_pt: u8, reduce_motion: bool, ctx: &egui::Context) {
    let density = Density::of_setting(density);
    let (light, dark, preference) = themes_of_setting(theme);
    for th in [light, dark] {
        let mut t = tokens(th, density);
        raise_text(&mut t, min_font_pt);
        if reduce_motion {
            t.motion_fast = 0;
            t.motion_base = 0;
        }
        let mut style = style_of(&t);
        if reduce_motion {
            style.scroll_animation = egui::style::ScrollAnimation::none();
        }
        ctx.set_style_of(th.egui(), Arc::new(style));
        install(ctx, &t);
    }
    ctx.set_theme(preference);
}

/// Raises every text token of `t` whose size is below `min_pt` points to that size, its line height by as much (the
/// setting `min_font_pt`); 0 leaves the tokens as they are.
fn raise_text(t: &mut Tokens, min_pt: u8) {
    let min = f32::from(min_pt);
    for token in [
        &mut t.type_caption,
        &mut t.type_small,
        &mut t.type_label,
        &mut t.type_body,
        &mut t.type_body_strong,
        &mut t.type_title,
        &mut t.type_display,
        &mut t.type_mono,
    ] {
        if token.size < min {
            token.line += min - token.size;
            token.size = min;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    /// Egui spreads a wheel line over several frames; with [`unsmoothed_scroll`] the whole line scrolls in the
    /// frame of the turn and nothing follows in the next one.
    #[test]
    fn reduced_motion_scrolls_a_wheel_line_in_its_frame() {
        let ctx = egui::Context::default();
        let line = ctx.options(|o| o.input_options.line_scroll_speed);
        let turn = || egui::RawInput {
            events: vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: Vec2::new(0.0, -1.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
                source: egui::MouseWheelSource::Wheel,
            }],
            ..Default::default()
        };
        let frame = |input: egui::RawInput, reduce: bool| {
            let mut delta = Vec2::ZERO;
            let mut out = ctx.run_ui(input, |ui| {
                if reduce {
                    unsmoothed_scroll(ui.ctx());
                }
                delta = ui.input(|i| i.smooth_scroll_delta);
            });
            out.textures_delta.clear();
            delta
        };
        let smoothed = frame(turn(), false);
        assert!(smoothed.y > -line, "egui alone spreads the line: {smoothed:?}");
        let _ = frame(egui::RawInput::default(), false);
        let _ = frame(egui::RawInput::default(), false);
        assert_eq!(
            frame(turn(), true),
            Vec2::new(0.0, -line),
            "the whole line in its frame"
        );
        assert_eq!(
            frame(egui::RawInput::default(), true),
            Vec2::ZERO,
            "nothing after it"
        );
    }

    #[test]
    fn a_setting_installs_both_slots_and_its_preference() {
        let ctx = egui::Context::default();
        apply_setting("hc-dark", "compact", 0, false, &ctx);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::Dark);
        assert_eq!(super::super::current(&ctx).theme, Theme::HighContrastDark);
        assert_eq!(super::super::current(&ctx).density, Density::Compact);
        apply_setting("system", "comfortable", 0, false, &ctx);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::System);
    }

    /// `min_font_pt` raises a text token below it to the minimum and its line height by as much,
    /// `type_body_strong` among them; a token at or above the minimum, and the setting 0, leave it as it is.
    #[test]
    fn raise_text_raises_the_size_and_the_line_height_by_as_much() {
        let base = tokens(Theme::Light, Density::Comfortable);
        let strong = base.type_body_strong;
        let display = base.type_display;
        let min = strong.size + 3.0;
        assert!(display.size >= min, "the display token stays above the minimum");
        let mut t = base.clone();
        raise_text(&mut t, min as u8);
        let min = f32::from(min as u8);
        assert_eq!(
            (t.type_body_strong.size, t.type_body_strong.line),
            (min, strong.line + (min - strong.size))
        );
        assert_eq!(
            (t.type_display.size, t.type_display.line),
            (display.size, display.line)
        );
        let mut unchanged = base.clone();
        raise_text(&mut unchanged, 0);
        assert_eq!(
            (unchanged.type_body_strong.size, unchanged.type_body_strong.line),
            (strong.size, strong.line)
        );
    }
}
