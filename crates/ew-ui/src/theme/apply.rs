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
    // FT-566: a 1-px caret (`border.width`, as the platforms draw it), so it clears the first glyph of the text or
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
    s.menu_margin = Margin::same(t.space_2 as i8);
    s.window_margin = Margin::same(t.space_5 as i8);
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

/// Installs `tokens` in `ctx`: the style of its egui slot (light or dark) and the theme preference for that slot, so
/// that `ctx.style()` is this style from the next pass.
pub fn apply(tokens: &Tokens, ctx: &egui::Context) {
    ctx.set_style_of(tokens.theme.egui(), Arc::new(style_of(tokens)));
    install(ctx, tokens);
    ctx.set_theme(tokens.theme.egui());
}

/// Installs the `theme` and `density` settings (`settings.toml`): both egui slots, then the preference, so that
/// `system` follows the operating system (D5).
pub fn apply_setting(theme: &str, density: &str, ctx: &egui::Context) {
    let density = Density::of_setting(density);
    let (light, dark, preference) = themes_of_setting(theme);
    for th in [light, dark] {
        let t = tokens(th, density);
        ctx.set_style_of(th.egui(), Arc::new(style_of(&t)));
        install(ctx, &t);
    }
    ctx.set_theme(preference);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn a_setting_installs_both_slots_and_its_preference() {
        let ctx = egui::Context::default();
        apply_setting("hc-dark", "compact", &ctx);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::Dark);
        assert_eq!(super::super::current(&ctx).theme, Theme::HighContrastDark);
        assert_eq!(super::super::current(&ctx).density, Density::Compact);
        apply_setting("system", "comfortable", &ctx);
        assert_eq!(ctx.options(|o| o.theme_preference), egui::ThemePreference::System);
    }
}
