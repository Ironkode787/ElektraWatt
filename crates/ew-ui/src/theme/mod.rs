// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The design language's one token source (WP-S2.17 part 1, D4; doc 16a §4): [`tokens`] gives every alias and
//! component token of a [`Theme`] and a [`Density`], [`apply`] maps them onto egui's `Style` (doc 16a §4.14), and
//! [`current`] gives widget code the tokens of the theme egui shows now. Widget code takes every colour, size and
//! radius from here; a source guard test fails on literals elsewhere in `ew-ui` (D4).
//!
//! **Themes.** Light, Dark, High contrast dark and High contrast light; the setting `system` (the default, D5) installs
//! Light and Dark as egui's light and dark styles and lets egui follow the operating system.

mod apply;
mod tokens;

use std::sync::Arc;

pub use apply::{apply, apply_setting, font, style_of};
pub use tokens::{
    Elevation, HC_PAIRS, PAIRS, Pair, PairKind, Rgba, ShadowLayer, TokenValue, Tokens, TypeToken, tokens,
};

/// A theme of the design language.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Theme {
    /// The reference theme of the mockup and the snapshots.
    Light,
    /// The mockup's dark theme.
    Dark,
    /// High contrast dark (MVP, doc 16a §4.7).
    HighContrastDark,
    /// High contrast light (Should).
    HighContrastLight,
}

impl Theme {
    /// Every theme, in the dump's order.
    pub const ALL: [Theme; 4] = [
        Theme::Light,
        Theme::Dark,
        Theme::HighContrastDark,
        Theme::HighContrastLight,
    ];

    /// The theme's word in `settings.toml` and the dump.
    pub fn word(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::HighContrastDark => "hc-dark",
            Theme::HighContrastLight => "hc-light",
        }
    }

    /// Whether egui shows it as a dark theme.
    pub fn is_dark(self) -> bool {
        matches!(self, Theme::Dark | Theme::HighContrastDark)
    }

    /// Whether it is one of the high-contrast themes.
    pub fn is_high_contrast(self) -> bool {
        matches!(self, Theme::HighContrastDark | Theme::HighContrastLight)
    }

    /// egui's light or dark slot for this theme.
    pub fn egui(self) -> egui::Theme {
        if self.is_dark() {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        }
    }
}

/// A density of the design language (doc 16a §4.13, D6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Density {
    /// 28-px rows, the 52-px command row (the default).
    Comfortable,
    /// 24-px rows, the 36-px command row.
    Compact,
}

impl Density {
    /// Both densities.
    pub const ALL: [Density; 2] = [Density::Comfortable, Density::Compact];

    /// The density's word in `settings.toml` and the dump.
    pub fn word(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
        }
    }

    /// The density of a `density` setting; anything else is Comfortable.
    pub fn of_setting(word: &str) -> Density {
        if word == "compact" {
            Density::Compact
        } else {
            Density::Comfortable
        }
    }
}

/// The themes a `theme` setting installs: `(light slot, dark slot, preference)`. `system` installs Light and Dark
/// and follows the operating system; a high-contrast theme takes its slot and pins it.
pub fn themes_of_setting(word: &str) -> (Theme, Theme, egui::ThemePreference) {
    match word {
        "light" => (Theme::Light, Theme::Dark, egui::ThemePreference::Light),
        "dark" => (Theme::Light, Theme::Dark, egui::ThemePreference::Dark),
        "hc-dark" => (Theme::Light, Theme::HighContrastDark, egui::ThemePreference::Dark),
        "hc-light" => (
            Theme::HighContrastLight,
            Theme::Dark,
            egui::ThemePreference::Light,
        ),
        _ => (Theme::Light, Theme::Dark, egui::ThemePreference::System),
    }
}

/// The WCAG 2.1 contrast ratio of two opaque colours (relative luminance, sRGB), from 1 to 21.
pub fn contrast(a: Rgba, b: Rgba) -> f32 {
    fn channel(c: u8) -> f64 {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    fn luminance(c: Rgba) -> f64 {
        0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
    }
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    ((hi + 0.05) / (lo + 0.05)) as f32
}

/// The tokens installed in a context: one per egui theme slot.
#[derive(Clone)]
struct Installed {
    light: Arc<Tokens>,
    dark: Arc<Tokens>,
}

fn installed_id() -> egui::Id {
    egui::Id::unique("ew-ui-theme-tokens")
}

pub(crate) fn install(ctx: &egui::Context, tokens: &Tokens) {
    let id = installed_id();
    ctx.data_mut(|d| {
        let mut slots = d.get_temp::<Installed>(id).unwrap_or_else(|| Installed {
            light: Arc::new(self::tokens(Theme::Light, tokens.density)),
            dark: Arc::new(self::tokens(Theme::Dark, tokens.density)),
        });
        if tokens.theme.is_dark() {
            slots.dark = Arc::new(tokens.clone());
        } else {
            slots.light = Arc::new(tokens.clone());
        }
        d.insert_temp(id, slots);
    });
}

/// The tokens of the theme `ctx` shows now (the installed ones, else Light or Dark at Comfortable).
pub fn current(ctx: &egui::Context) -> Arc<Tokens> {
    let dark = ctx.theme() == egui::Theme::Dark;
    ctx.data(|d| d.get_temp::<Installed>(installed_id()))
        .map(|s| if dark { s.dark } else { s.light })
        .unwrap_or_else(|| {
            Arc::new(tokens(
                if dark { Theme::Dark } else { Theme::Light },
                Density::Comfortable,
            ))
        })
}

/// The tint of an image painted as it is (no tint).
pub const NO_TINT: egui::Color32 = egui::Color32::WHITE;

/// A square corner (`radius.none`).
pub const SQUARE: egui::CornerRadius = egui::CornerRadius::ZERO;

/// No colour (an outline-only shape's fill).
pub const NONE: egui::Color32 = egui::Color32::TRANSPARENT;

/// An egui stroke of `width` px in `colour`.
pub fn stroke(width: f32, colour: Rgba) -> egui::Stroke {
    egui::Stroke::new(width, colour.color32())
}

/// An egui corner radius of `radius` px (`radius.pill` clamps to egui's 255).
pub fn radius(radius: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(radius.clamp(0.0, 255.0).round() as u8)
}

/// One side of an egui margin of `px` points: rounded and clamped to egui's `i8` range (FT-816: a token cast with
/// `as` would silently saturate).
pub fn margin_px(px: f32) -> i8 {
    px.round().clamp(f32::from(i8::MIN), f32::from(i8::MAX)) as i8
}

/// An egui shadow from an elevation's key layer.
pub fn shadow(elevation: &Elevation) -> egui::Shadow {
    let l = elevation.layers[0];
    egui::Shadow {
        offset: [l.x, l.y],
        blur: l.blur,
        spread: l.spread,
        color: l.colour.color32(),
    }
}

/// A colour from 8-bit channels and a straight alpha, for colours that come from data (a highlight slot of the
/// viewmodel, an overlay colour of the renderer), never literals.
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 {
    Rgba { r, g, b, a }.color32()
}
