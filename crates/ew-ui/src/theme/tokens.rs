// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tokens of doc 16a §4 in three layers (§4.1): the global ramps (`ramp`, raw values that widget code never
//! names), the alias tokens per theme ([`Tokens`]' colour fields) and the component tokens per density (its size
//! fields). [`tokens`] builds them; [`Tokens::entries`] lists every token by its dump name (the names of doc 16a),
//! which `tests/expected/tokens.txt` blesses.

use super::{Density, Theme};

/// A colour token: sRGB bytes with a straight (not premultiplied) alpha, so that `rgba(26,29,33,.06)` keeps its
/// channels exactly.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 opaque).
    pub a: u8,
}

impl Rgba {
    /// An opaque colour from `0xRRGGBB`.
    pub const fn hex(v: u32) -> Rgba {
        Rgba {
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
            a: 255,
        }
    }

    /// The same colour with alpha `a`.
    pub const fn alpha(self, a: u8) -> Rgba {
        Rgba { a, ..self }
    }

    /// Fully transparent.
    pub const TRANSPARENT: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// The colour as egui paints it.
    pub fn color32(self) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(self.r, self.g, self.b, self.a)
    }

    /// `self` painted over the opaque `under` (source-over), opaque.
    pub fn over(self, under: Rgba) -> Rgba {
        let a = u32::from(self.a);
        let mix = |s: u8, d: u8| ((u32::from(s) * a + u32::from(d) * (255 - a) + 127) / 255) as u8;
        Rgba {
            r: mix(self.r, under.r),
            g: mix(self.g, under.g),
            b: mix(self.b, under.b),
            a: 255,
        }
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
    pub fn text(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

/// One layer of a shadow, as CSS writes it: `x y blur spread colour`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowLayer {
    /// Horizontal offset in px.
    pub x: i8,
    /// Vertical offset in px.
    pub y: i8,
    /// Blur radius in px.
    pub blur: u8,
    /// Spread in px.
    pub spread: u8,
    /// Colour.
    pub colour: Rgba,
}

/// An elevation token (doc 16a §4.10): two shadow layers, the key shadow first. The high-contrast themes have none
/// (both layers transparent; borders replace them, §4.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Elevation {
    /// The layers, key shadow first.
    pub layers: [ShadowLayer; 2],
}

impl Elevation {
    const NONE: Elevation = Elevation {
        layers: [ShadowLayer {
            x: 0,
            y: 0,
            blur: 0,
            spread: 0,
            colour: Rgba::TRANSPARENT,
        }; 2],
    };

    fn text(&self) -> String {
        self.layers
            .iter()
            .map(|l| format!("{} {} {} {} {}", l.x, l.y, l.blur, l.spread, l.colour.text()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A type token (doc 16a §4.8): size and line height in px at 100 %, the weight and the case.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TypeToken {
    /// Font size in px.
    pub size: f32,
    /// Line height in px.
    pub line: f32,
    /// SemiBold (600) rather than Regular (400).
    pub semibold: bool,
    /// Upper case with +0.02 em tracking (`type.label`).
    pub upper: bool,
}

impl TypeToken {
    const fn regular(size: f32, line: f32) -> TypeToken {
        TypeToken {
            size,
            line,
            semibold: false,
            upper: false,
        }
    }

    const fn semibold(size: f32, line: f32) -> TypeToken {
        TypeToken {
            size,
            line,
            semibold: true,
            upper: false,
        }
    }

    fn text(&self) -> String {
        format!(
            "{}/{} {}{}",
            self.size,
            self.line,
            if self.semibold { "semibold" } else { "regular" },
            if self.upper { " upper" } else { "" }
        )
    }
}

/// The global ramps of doc 16a §4.2 to §4.4 and the high-contrast starting values of §4.7.
// The whole ramp is the global layer (§4.1): not every step has an alias yet.
#[allow(dead_code)]
pub(crate) mod ramp {
    use super::Rgba;

    pub const SLATE_0: Rgba = Rgba::hex(0xFFFFFF);
    pub const SLATE_25: Rgba = Rgba::hex(0xFAFBFC);
    pub const SLATE_50: Rgba = Rgba::hex(0xF4F5F7);
    pub const SLATE_100: Rgba = Rgba::hex(0xECEEF1);
    pub const SLATE_150: Rgba = Rgba::hex(0xE3E6EA);
    pub const SLATE_200: Rgba = Rgba::hex(0xD5D9DF);
    pub const SLATE_300: Rgba = Rgba::hex(0xBBC1C9);
    pub const SLATE_350: Rgba = Rgba::hex(0xA7AEB8);
    pub const SLATE_400: Rgba = Rgba::hex(0x868E99);
    pub const SLATE_450: Rgba = Rgba::hex(0x959DA8);
    pub const SLATE_500: Rgba = Rgba::hex(0x646C78);
    pub const SLATE_600: Rgba = Rgba::hex(0x555C66);
    pub const SLATE_700: Rgba = Rgba::hex(0x3F454D);
    pub const SLATE_750: Rgba = Rgba::hex(0x343941);
    pub const SLATE_800: Rgba = Rgba::hex(0x2A2E34);
    pub const SLATE_850: Rgba = Rgba::hex(0x212429);
    pub const SLATE_900: Rgba = Rgba::hex(0x1A1D21);
    pub const SLATE_950: Rgba = Rgba::hex(0x121417);

    pub const VOLT_50: Rgba = Rgba::hex(0xE7F0FC);
    pub const VOLT_250: Rgba = Rgba::hex(0x77B3FF);
    pub const VOLT_300: Rgba = Rgba::hex(0x5AA2FF);
    pub const VOLT_500: Rgba = Rgba::hex(0x4A90EA);
    pub const VOLT_600: Rgba = Rgba::hex(0x0B63CE);
    pub const VOLT_700: Rgba = Rgba::hex(0x0A58B8);
    pub const VOLT_800: Rgba = Rgba::hex(0x094C9F);
    pub const VOLT_900: Rgba = Rgba::hex(0x203754);
    pub const VOLT_950: Rgba = Rgba::hex(0x0B1A33);

    pub const ERROR_LIGHT: Rgba = Rgba::hex(0xC42B3C);
    pub const ERROR_SUBTLE_LIGHT: Rgba = Rgba::hex(0xFDECEE);
    pub const ERROR_DARK: Rgba = Rgba::hex(0xFF8A8A);
    pub const ERROR_SUBTLE_DARK: Rgba = Rgba::hex(0x3A2226);
    pub const WARNING_LIGHT: Rgba = Rgba::hex(0x9A5B00);
    pub const WARNING_ICON_LIGHT: Rgba = Rgba::hex(0xB26A00);
    pub const WARNING_SUBTLE_LIGHT: Rgba = Rgba::hex(0xFFF4E0);
    pub const WARNING_DARK: Rgba = Rgba::hex(0xF5B544);
    pub const WARNING_SUBTLE_DARK: Rgba = Rgba::hex(0x3A2E17);
    pub const SUCCESS_LIGHT: Rgba = Rgba::hex(0x17733F);
    pub const SUCCESS_SUBTLE_LIGHT: Rgba = Rgba::hex(0xE8F5EE);
    pub const SUCCESS_DARK: Rgba = Rgba::hex(0x47C97E);
    pub const SUCCESS_SUBTLE_DARK: Rgba = Rgba::hex(0x1D3326);

    /// The shadow ink of the light theme (`rgba(16,24,40,…)`).
    pub const INK: Rgba = Rgba::hex(0x101828);
    pub const BLACK: Rgba = Rgba::hex(0x000000);
    pub const WHITE: Rgba = Rgba::hex(0xFFFFFF);

    /// Doc 16 §10.3's highlight hues (doc 16a §4.6, D14).
    pub const VERMILLION: Rgba = Rgba::hex(0xD55E00);
    pub const BLUISH_GREEN: Rgba = Rgba::hex(0x009E73);
    pub const REDDISH_PURPLE: Rgba = Rgba::hex(0xCC79A7);
    pub const DARK_ORANGE: Rgba = Rgba::hex(0xA86F00);

    pub const GRID_DOT: Rgba = Rgba::hex(0xB8BEC7);
    pub const SURROUND_DARK: Rgba = Rgba::hex(0x2E3238);

    // High contrast (doc 16a §4.7's starting values, moved only where the token test needs it).
    pub const HC_DARK_SECONDARY: Rgba = Rgba::hex(0xD7DBE0);
    pub const HC_DARK_ACCENT: Rgba = Rgba::hex(0x79B8FF);
    pub const HC_DARK_ACCENT_HOVER: Rgba = Rgba::hex(0xA3CEFF);
    pub const HC_DARK_ACCENT_SUBTLE: Rgba = Rgba::hex(0x001A33);
    pub const HC_DARK_HOVER: Rgba = Rgba::hex(0x262626);
    pub const HC_DARK_PRESSED: Rgba = Rgba::hex(0x3A3A3A);
    pub const HC_DARK_DISABLED: Rgba = Rgba::hex(0x8A9099);
    pub const HC_DARK_ERROR: Rgba = Rgba::hex(0xFF9C9C);
    pub const HC_DARK_WARNING: Rgba = Rgba::hex(0xFFD066);
    pub const HC_DARK_SUCCESS: Rgba = Rgba::hex(0x6EE29E);
    pub const HC_DARK_SURROUND: Rgba = Rgba::hex(0x1A1A1A);
    pub const HC_LIGHT_SECONDARY: Rgba = Rgba::hex(0x2A2E34);
    pub const HC_LIGHT_ACCENT: Rgba = Rgba::hex(0x0047A3);
    pub const HC_LIGHT_ACCENT_HOVER: Rgba = Rgba::hex(0x003A85);
    pub const HC_LIGHT_ACCENT_SUBTLE: Rgba = Rgba::hex(0xE6F0FF);
    pub const HC_LIGHT_HOVER: Rgba = Rgba::hex(0xE6E6E6);
    pub const HC_LIGHT_PRESSED: Rgba = Rgba::hex(0xCCCCCC);
    pub const HC_LIGHT_DISABLED: Rgba = Rgba::hex(0x6B717A);
    pub const HC_LIGHT_ERROR: Rgba = Rgba::hex(0xA1001D);
    pub const HC_LIGHT_WARNING: Rgba = Rgba::hex(0x6B3F00);
    pub const HC_LIGHT_SUCCESS: Rgba = Rgba::hex(0x0B5A2E);
    pub const HC_LIGHT_SURROUND: Rgba = Rgba::hex(0xC8C8C8);
}

/// Every alias and component token of doc 16a §4 for one theme and density. Field names follow the dump names
/// (`color.text.secondary` is `text_secondary`); [`Tokens::entries`] gives the dump names.
#[derive(Clone, PartialEq, Debug)]
#[allow(missing_docs)]
pub struct Tokens {
    /// The theme these tokens are for.
    pub theme: Theme,
    /// The density these tokens are for.
    pub density: Density,

    // Colour aliases (§4.5).
    pub surface_chrome: Rgba,
    pub surface_panel: Rgba,
    pub surface_raised: Rgba,
    pub surface_sunken: Rgba,
    pub surface_inverse: Rgba,
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_tertiary: Rgba,
    pub text_disabled: Rgba,
    pub text_on_accent: Rgba,
    pub text_on_inverse: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_pressed: Rgba,
    pub accent_subtle: Rgba,
    pub border_subtle: Rgba,
    pub border_default: Rgba,
    pub border_control: Rgba,
    pub focus: Rgba,
    pub state_hover: Rgba,
    pub state_pressed: Rgba,
    pub error: Rgba,
    pub error_subtle: Rgba,
    pub warning: Rgba,
    pub warning_icon: Rgba,
    pub warning_subtle: Rgba,
    pub success: Rgba,
    pub success_subtle: Rgba,
    pub info: Rgba,
    pub info_subtle: Rgba,
    /// The dialog backdrop over the whole window (§6.12: light `slate-900` at 24 %, dark black at 40 %, FT-557).
    pub backdrop: Rgba,

    // Canvas tokens (§4.6); the sheet is white in every theme.
    pub canvas_surround: Rgba,
    pub canvas_sheet: Rgba,
    pub canvas_sheet_line: Rgba,
    pub canvas_grid_dot: Rgba,
    pub canvas_selection: Rgba,
    pub canvas_hover: Rgba,
    pub canvas_ghost: Rgba,
    pub canvas_snap: Rgba,
    pub canvas_highlight: [Rgba; 4],
    pub canvas_measure: Rgba,
    pub canvas_stale: Rgba,
    pub canvas_chip: Rgba,
    pub canvas_chip_text: Rgba,
    pub canvas_highlight_label: Rgba,

    // Elevation (§4.10).
    pub elevation_1: Elevation,
    pub elevation_2: Elevation,
    /// The window and popup outline: 0 (none) in light and dark, 1 px in the high-contrast themes (§4.7).
    pub border_raised_width: f32,

    // Typography (§4.8).
    pub type_caption: TypeToken,
    pub type_small: TypeToken,
    pub type_label: TypeToken,
    pub type_body: TypeToken,
    pub type_body_strong: TypeToken,
    pub type_title: TypeToken,
    pub type_display: TypeToken,
    /// `Monospace` in egui: Noto Sans at 12 (§4.14).
    pub type_mono: TypeToken,

    // Spacing (§4.9).
    pub space_1: f32,
    pub space_2: f32,
    pub space_icon: f32,
    pub space_3: f32,
    pub space_4: f32,
    pub space_5: f32,
    pub space_6: f32,
    pub space_7: f32,

    // Sizes (§4.9) and component sizes (§6).
    pub size_row: f32,
    pub size_control: f32,
    pub size_control_inline: f32,
    pub size_tab_row: f32,
    pub size_command_row: f32,
    pub size_command_large_height: f32,
    pub size_command_large_min_width: f32,
    pub size_command_small: f32,
    pub size_quick_access_width: f32,
    pub size_quick_access_height: f32,
    pub size_search_width: f32,
    pub size_search_height: f32,
    pub size_panel_header: f32,
    pub size_dock_tab_bar: f32,
    pub size_rail: f32,
    pub size_rail_button: f32,
    pub size_status: f32,
    pub size_status_field: f32,
    pub size_panel_left: f32,
    pub size_panel_right: f32,
    pub size_dock_bottom: f32,
    /// The bottom dock is never shown lower than this (W31-1: at least three Messages rows and a detail block).
    pub size_dock_bottom_min: f32,
    pub size_property_label: f32,
    pub size_tree_indent: f32,
    pub size_table_header: f32,
    pub size_badge: f32,
    pub size_chip: f32,
    pub size_button_min_width: f32,
    pub size_dialog_small: f32,
    pub size_dialog_form: f32,
    pub size_palette: f32,
    pub size_palette_row: f32,
    pub size_toast: f32,
    pub size_tooltip_max: f32,
    // Component tokens of the built widgets (doc 16a v1.1 §4.15).
    pub size_table_column: f32,
    pub size_table_column_min: f32,
    pub size_table_column_max: f32,
    pub size_severity_icon: f32,
    pub size_chooser_min_width: f32,
    pub size_chooser_max_width: f32,
    /// The chooser list shows at most this many rows (`size.row` each).
    pub size_chooser_rows: u32,
    /// A Preview list shows this many rows before it scrolls.
    pub size_preview_list_rows: u32,
    pub size_card_strip: f32,
    pub table_frozen_edge: Rgba,
    pub table_cursor: Rgba,
    pub table_cursor_idle: Rgba,
    pub history_undone: Rgba,
    pub card_strip: Rgba,

    // Radii and borders (§4.10).
    pub radius_none: f32,
    pub radius_mark: f32,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub radius_pill: f32,
    pub border_width: f32,
    pub focus_width: f32,

    // Icons (§4.11).
    pub icon_sm: f32,
    pub icon_md: f32,
    pub icon_lg: f32,
    pub icon_stroke: f32,
    pub icon_stroke_lg: f32,

    // Motion (§4.12), in milliseconds.
    pub motion_fast: u32,
    pub motion_base: u32,
}

/// A token's value, for the dump and the lookups of the tests.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TokenValue {
    /// A colour.
    Colour(Rgba),
    /// A length in px.
    Px(f32),
    /// An elevation.
    Shadow(Elevation),
    /// A type token.
    Type(TypeToken),
    /// A duration in ms.
    Ms(u32),
    /// A count (rows).
    Count(u32),
}

impl TokenValue {
    /// The value as the dump writes it.
    pub fn text(&self) -> String {
        match self {
            TokenValue::Colour(c) => c.text(),
            TokenValue::Px(v) => format!("{v}px"),
            TokenValue::Shadow(e) => e.text(),
            TokenValue::Type(t) => t.text(),
            TokenValue::Ms(v) => format!("{v}ms"),
            TokenValue::Count(v) => format!("{v}"),
        }
    }
}

/// What a contrast pair asserts (doc 16a §4.5, §4.6; NFR-A11Y-03, NFR-A11Y-04).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PairKind {
    /// Text: 4.5:1, 7:1 in the high-contrast themes.
    Text,
    /// Icons, boundaries, focus: 3:1.
    NonText,
    /// Text on the sheet (a highlight name chip's label): 4.5:1 in every theme, since the sheet does not change.
    CanvasText,
    /// A canvas overlay slot on the white sheet: 3:1.
    Canvas,
}

/// A declared contrast pair: the foreground and background tokens by dump name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pair {
    /// The foreground token.
    pub fg: &'static str,
    /// The background token.
    pub bg: &'static str,
    /// What it must reach.
    pub kind: PairKind,
}

impl Pair {
    /// The ratio this pair must reach in `theme`.
    pub fn floor(&self, theme: Theme) -> f32 {
        match self.kind {
            PairKind::Text if theme.is_high_contrast() => 7.0,
            PairKind::Text => 4.5,
            PairKind::NonText | PairKind::Canvas => 3.0,
            PairKind::CanvasText => 4.5,
        }
    }
}

const fn pair(fg: &'static str, bg: &'static str, kind: PairKind) -> Pair {
    Pair { fg, bg, kind }
}

/// The pairs of doc 16a §4.5 (the "On" column and the pairs named under the table), §4.6 (the canvas slots on the
/// white sheet, the highlight labels on their hues) and §4.7 (every boundary of the high-contrast themes).
pub const PAIRS: &[Pair] = &[
    pair("color.text.primary", "color.surface.panel", PairKind::Text),
    pair("color.text.primary", "color.surface.chrome", PairKind::Text),
    pair("color.text.primary", "color.surface.raised", PairKind::Text),
    pair("color.text.primary", "color.surface.sunken", PairKind::Text),
    pair("color.text.primary", "color.accent.subtle", PairKind::Text),
    pair("color.text.secondary", "color.surface.panel", PairKind::Text),
    pair("color.text.secondary", "color.surface.chrome", PairKind::Text),
    pair("color.text.secondary", "color.surface.raised", PairKind::Text),
    // §4.15: a selected or marked row draws its tertiary parts in secondary text.
    pair("color.text.secondary", "color.accent.subtle", PairKind::Text),
    pair("color.history.undone", "color.surface.panel", PairKind::Text),
    pair("color.table.cursor", "color.surface.panel", PairKind::NonText),
    pair("color.table.cursor", "color.accent.subtle", PairKind::NonText),
    pair(
        "color.table.cursor_idle",
        "color.accent.subtle",
        PairKind::NonText,
    ),
    pair("color.text.tertiary", "color.surface.chrome", PairKind::Text),
    pair("color.text.tertiary", "color.surface.panel", PairKind::Text),
    pair("color.text.tertiary", "color.surface.raised", PairKind::Text),
    pair("color.text.on_accent", "color.accent", PairKind::Text),
    pair("color.text.on_inverse", "color.surface.inverse", PairKind::Text),
    pair("color.accent", "color.surface.panel", PairKind::Text),
    pair("color.accent", "color.accent.subtle", PairKind::Text),
    pair("color.error", "color.surface.panel", PairKind::Text),
    pair("color.warning", "color.surface.panel", PairKind::Text),
    pair("color.success", "color.surface.panel", PairKind::Text),
    pair("color.warning", "color.warning.subtle", PairKind::Text),
    pair("color.border.control", "color.surface.panel", PairKind::NonText),
    pair("color.focus", "color.surface.panel", PairKind::NonText),
    pair("color.warning.icon", "color.surface.panel", PairKind::NonText),
    pair("canvas.selection", "canvas.sheet", PairKind::Canvas),
    pair("canvas.hover", "canvas.sheet", PairKind::Canvas),
    pair("canvas.snap", "canvas.sheet", PairKind::Canvas),
    pair("canvas.highlight.1", "canvas.sheet", PairKind::Canvas),
    pair("canvas.highlight.2", "canvas.sheet", PairKind::Canvas),
    pair("canvas.highlight.3", "canvas.sheet", PairKind::Canvas),
    pair("canvas.highlight.4", "canvas.sheet", PairKind::Canvas),
    pair("canvas.measure", "canvas.sheet", PairKind::Canvas),
    pair(
        "canvas.highlight.label",
        "canvas.highlight.1",
        PairKind::CanvasText,
    ),
    pair(
        "canvas.highlight.label",
        "canvas.highlight.2",
        PairKind::CanvasText,
    ),
    pair(
        "canvas.highlight.label",
        "canvas.highlight.3",
        PairKind::CanvasText,
    ),
    pair(
        "canvas.highlight.label",
        "canvas.highlight.4",
        PairKind::CanvasText,
    ),
];

/// The boundaries every high-contrast theme must show at 3:1 (§4.7).
pub const HC_PAIRS: &[Pair] = &[
    pair("color.border.subtle", "color.surface.panel", PairKind::NonText),
    pair("color.border.default", "color.surface.panel", PairKind::NonText),
    pair("color.border.default", "color.surface.chrome", PairKind::NonText),
    pair("color.text.disabled", "color.surface.panel", PairKind::NonText),
];

impl Tokens {
    /// Every token by its dump name, in the order of doc 16a §4.
    pub fn entries(&self) -> Vec<(&'static str, TokenValue)> {
        use TokenValue::{Colour, Count, Ms, Px, Shadow, Type};
        vec![
            ("color.surface.chrome", Colour(self.surface_chrome)),
            ("color.surface.panel", Colour(self.surface_panel)),
            ("color.surface.raised", Colour(self.surface_raised)),
            ("color.surface.sunken", Colour(self.surface_sunken)),
            ("color.surface.inverse", Colour(self.surface_inverse)),
            ("color.text.primary", Colour(self.text_primary)),
            ("color.text.secondary", Colour(self.text_secondary)),
            ("color.text.tertiary", Colour(self.text_tertiary)),
            ("color.text.disabled", Colour(self.text_disabled)),
            ("color.text.on_accent", Colour(self.text_on_accent)),
            ("color.text.on_inverse", Colour(self.text_on_inverse)),
            ("color.accent", Colour(self.accent)),
            ("color.accent.hover", Colour(self.accent_hover)),
            ("color.accent.pressed", Colour(self.accent_pressed)),
            ("color.accent.subtle", Colour(self.accent_subtle)),
            ("color.border.subtle", Colour(self.border_subtle)),
            ("color.border.default", Colour(self.border_default)),
            ("color.border.control", Colour(self.border_control)),
            ("color.focus", Colour(self.focus)),
            ("color.state.hover", Colour(self.state_hover)),
            ("color.state.pressed", Colour(self.state_pressed)),
            ("color.error", Colour(self.error)),
            ("color.error.subtle", Colour(self.error_subtle)),
            ("color.warning", Colour(self.warning)),
            ("color.warning.icon", Colour(self.warning_icon)),
            ("color.warning.subtle", Colour(self.warning_subtle)),
            ("color.success", Colour(self.success)),
            ("color.success.subtle", Colour(self.success_subtle)),
            ("color.info", Colour(self.info)),
            ("color.info.subtle", Colour(self.info_subtle)),
            ("color.backdrop", Colour(self.backdrop)),
            ("canvas.surround", Colour(self.canvas_surround)),
            ("canvas.sheet", Colour(self.canvas_sheet)),
            ("canvas.sheet_line", Colour(self.canvas_sheet_line)),
            ("canvas.grid_dot", Colour(self.canvas_grid_dot)),
            ("canvas.selection", Colour(self.canvas_selection)),
            ("canvas.hover", Colour(self.canvas_hover)),
            ("canvas.ghost", Colour(self.canvas_ghost)),
            ("canvas.snap", Colour(self.canvas_snap)),
            ("canvas.highlight.1", Colour(self.canvas_highlight[0])),
            ("canvas.highlight.2", Colour(self.canvas_highlight[1])),
            ("canvas.highlight.3", Colour(self.canvas_highlight[2])),
            ("canvas.highlight.4", Colour(self.canvas_highlight[3])),
            ("canvas.measure", Colour(self.canvas_measure)),
            ("canvas.stale", Colour(self.canvas_stale)),
            ("canvas.chip", Colour(self.canvas_chip)),
            ("canvas.chip_text", Colour(self.canvas_chip_text)),
            ("canvas.highlight.label", Colour(self.canvas_highlight_label)),
            ("elevation.1", Shadow(self.elevation_1)),
            ("elevation.2", Shadow(self.elevation_2)),
            ("border.raised", Px(self.border_raised_width)),
            ("type.caption", Type(self.type_caption)),
            ("type.small", Type(self.type_small)),
            ("type.label", Type(self.type_label)),
            ("type.body", Type(self.type_body)),
            ("type.body.strong", Type(self.type_body_strong)),
            ("type.title", Type(self.type_title)),
            ("type.display", Type(self.type_display)),
            ("type.mono", Type(self.type_mono)),
            ("space.1", Px(self.space_1)),
            ("space.2", Px(self.space_2)),
            ("space.icon", Px(self.space_icon)),
            ("space.3", Px(self.space_3)),
            ("space.4", Px(self.space_4)),
            ("space.5", Px(self.space_5)),
            ("space.6", Px(self.space_6)),
            ("space.7", Px(self.space_7)),
            ("size.row", Px(self.size_row)),
            ("size.control", Px(self.size_control)),
            ("size.control.inline", Px(self.size_control_inline)),
            ("size.tab_row", Px(self.size_tab_row)),
            ("size.command_row", Px(self.size_command_row)),
            ("size.command.large.height", Px(self.size_command_large_height)),
            (
                "size.command.large.min_width",
                Px(self.size_command_large_min_width),
            ),
            ("size.command.small", Px(self.size_command_small)),
            ("size.quick_access.width", Px(self.size_quick_access_width)),
            ("size.quick_access.height", Px(self.size_quick_access_height)),
            ("size.search.width", Px(self.size_search_width)),
            ("size.search.height", Px(self.size_search_height)),
            ("size.panel_header", Px(self.size_panel_header)),
            ("size.dock_tab_bar", Px(self.size_dock_tab_bar)),
            ("size.rail", Px(self.size_rail)),
            ("size.rail_button", Px(self.size_rail_button)),
            ("size.status", Px(self.size_status)),
            ("size.status_field", Px(self.size_status_field)),
            ("size.panel.left", Px(self.size_panel_left)),
            ("size.panel.right", Px(self.size_panel_right)),
            ("size.dock.bottom", Px(self.size_dock_bottom)),
            ("size.dock.bottom_min", Px(self.size_dock_bottom_min)),
            ("size.property_label", Px(self.size_property_label)),
            ("size.tree_indent", Px(self.size_tree_indent)),
            ("size.table_header", Px(self.size_table_header)),
            ("size.badge", Px(self.size_badge)),
            ("size.chip", Px(self.size_chip)),
            ("size.button.min_width", Px(self.size_button_min_width)),
            ("size.dialog.small", Px(self.size_dialog_small)),
            ("size.dialog.form", Px(self.size_dialog_form)),
            ("size.palette", Px(self.size_palette)),
            ("size.palette.row", Px(self.size_palette_row)),
            ("size.toast", Px(self.size_toast)),
            ("size.tooltip.max", Px(self.size_tooltip_max)),
            ("size.table_column", Px(self.size_table_column)),
            ("size.table_column.min", Px(self.size_table_column_min)),
            ("size.table_column.max", Px(self.size_table_column_max)),
            ("size.severity_icon", Px(self.size_severity_icon)),
            ("size.chooser.min_width", Px(self.size_chooser_min_width)),
            ("size.chooser.max_width", Px(self.size_chooser_max_width)),
            ("size.chooser.rows", Count(self.size_chooser_rows)),
            ("size.preview_list.rows", Count(self.size_preview_list_rows)),
            ("size.card_strip", Px(self.size_card_strip)),
            ("color.table.frozen_edge", Colour(self.table_frozen_edge)),
            ("color.table.cursor", Colour(self.table_cursor)),
            ("color.table.cursor_idle", Colour(self.table_cursor_idle)),
            ("color.history.undone", Colour(self.history_undone)),
            ("color.card.strip", Colour(self.card_strip)),
            ("radius.none", Px(self.radius_none)),
            ("radius.mark", Px(self.radius_mark)),
            ("radius.sm", Px(self.radius_sm)),
            ("radius.md", Px(self.radius_md)),
            ("radius.lg", Px(self.radius_lg)),
            ("radius.pill", Px(self.radius_pill)),
            ("border.width", Px(self.border_width)),
            ("focus.width", Px(self.focus_width)),
            ("icon.sm", Px(self.icon_sm)),
            ("icon.md", Px(self.icon_md)),
            ("icon.lg", Px(self.icon_lg)),
            ("icon.stroke", Px(self.icon_stroke)),
            ("icon.stroke.lg", Px(self.icon_stroke_lg)),
            ("motion.fast", Ms(self.motion_fast)),
            ("motion.base", Ms(self.motion_base)),
        ]
    }

    /// The token named `name` (a dump name).
    pub fn get(&self, name: &str) -> Option<TokenValue> {
        self.entries()
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v)
    }

    /// The colour token named `name`.
    pub fn colour(&self, name: &str) -> Option<Rgba> {
        match self.get(name)? {
            TokenValue::Colour(c) => Some(c),
            _ => None,
        }
    }

    /// The pairs this theme asserts: [`PAIRS`], and [`HC_PAIRS`] in the high-contrast themes.
    pub fn pairs(&self) -> Vec<Pair> {
        let mut out = PAIRS.to_vec();
        if self.theme.is_high_contrast() {
            out.extend_from_slice(HC_PAIRS);
        }
        out
    }
}

/// The colour aliases of one theme.
struct Palette {
    chrome: Rgba,
    panel: Rgba,
    raised: Rgba,
    sunken: Rgba,
    inverse: Rgba,
    primary: Rgba,
    secondary: Rgba,
    tertiary: Rgba,
    disabled: Rgba,
    on_accent: Rgba,
    on_inverse: Rgba,
    accent: Rgba,
    accent_hover: Rgba,
    accent_pressed: Rgba,
    accent_subtle: Rgba,
    border_subtle: Rgba,
    border_default: Rgba,
    border_control: Rgba,
    hover: Rgba,
    pressed: Rgba,
    error: Rgba,
    error_subtle: Rgba,
    warning: Rgba,
    warning_icon: Rgba,
    warning_subtle: Rgba,
    success: Rgba,
    success_subtle: Rgba,
    surround: Rgba,
    backdrop: Rgba,
    elevation_1: Elevation,
    elevation_2: Elevation,
    raised_border: f32,
}

const fn layer(x: i8, y: i8, blur: u8, spread: u8, colour: Rgba) -> ShadowLayer {
    ShadowLayer {
        x,
        y,
        blur,
        spread,
        colour,
    }
}

fn palette(theme: Theme) -> Palette {
    use ramp::*;
    match theme {
        Theme::Light => Palette {
            chrome: SLATE_50,
            panel: SLATE_0,
            raised: SLATE_0,
            sunken: SLATE_50,
            inverse: SLATE_900,
            primary: SLATE_900,
            secondary: SLATE_600,
            tertiary: SLATE_500,
            disabled: SLATE_350,
            on_accent: WHITE,
            on_inverse: SLATE_100,
            accent: VOLT_600,
            accent_hover: VOLT_700,
            accent_pressed: VOLT_800,
            accent_subtle: VOLT_50,
            border_subtle: SLATE_150,
            border_default: SLATE_200,
            border_control: SLATE_400,
            // slate-900 at 6 % and 10 % (the mockup's rgba(26,29,33,.06) and .10).
            hover: SLATE_900.alpha(15),
            pressed: SLATE_900.alpha(26),
            error: ERROR_LIGHT,
            error_subtle: ERROR_SUBTLE_LIGHT,
            warning: WARNING_LIGHT,
            warning_icon: WARNING_ICON_LIGHT,
            warning_subtle: WARNING_SUBTLE_LIGHT,
            success: SUCCESS_LIGHT,
            success_subtle: SUCCESS_SUBTLE_LIGHT,
            surround: SLATE_150,
            // slate-900 at 24 % (§6.12).
            backdrop: SLATE_900.alpha(61),
            elevation_1: Elevation {
                layers: [layer(0, 1, 2, 0, INK.alpha(26)), layer(0, 0, 0, 1, INK.alpha(10))],
            },
            elevation_2: Elevation {
                layers: [
                    layer(0, 8, 24, 0, INK.alpha(41)),
                    layer(0, 2, 6, 0, INK.alpha(20)),
                ],
            },
            raised_border: 0.0,
        },
        Theme::Dark => Palette {
            chrome: SLATE_900,
            panel: SLATE_850,
            raised: SLATE_800,
            sunken: SLATE_900,
            inverse: SLATE_100,
            primary: SLATE_100,
            secondary: SLATE_350,
            tertiary: SLATE_450,
            disabled: SLATE_600,
            on_accent: VOLT_950,
            on_inverse: SLATE_900,
            accent: VOLT_300,
            accent_hover: VOLT_250,
            accent_pressed: VOLT_500,
            accent_subtle: VOLT_900,
            border_subtle: SLATE_750,
            border_default: SLATE_700,
            border_control: SLATE_400,
            // white at 7 % and 12 %.
            hover: WHITE.alpha(18),
            pressed: WHITE.alpha(31),
            error: ERROR_DARK,
            error_subtle: ERROR_SUBTLE_DARK,
            warning: WARNING_DARK,
            warning_icon: WARNING_DARK,
            warning_subtle: WARNING_SUBTLE_DARK,
            success: SUCCESS_DARK,
            success_subtle: SUCCESS_SUBTLE_DARK,
            surround: SURROUND_DARK,
            // FT-557: black at 40 % (the alpha of the dark `elevation.1` shadow); slate-900 at 24 % left the chrome
            // (itself slate-900) and the panels undimmed.
            backdrop: BLACK.alpha(102),
            elevation_1: Elevation {
                layers: [
                    layer(0, 1, 2, 0, BLACK.alpha(102)),
                    layer(0, 0, 0, 1, WHITE.alpha(10)),
                ],
            },
            elevation_2: Elevation {
                layers: [
                    layer(0, 8, 24, 0, BLACK.alpha(128)),
                    layer(0, 2, 6, 0, BLACK.alpha(77)),
                ],
            },
            raised_border: 0.0,
        },
        Theme::HighContrastDark => Palette {
            chrome: BLACK,
            panel: BLACK,
            raised: BLACK,
            sunken: BLACK,
            inverse: WHITE,
            primary: WHITE,
            secondary: HC_DARK_SECONDARY,
            tertiary: HC_DARK_SECONDARY,
            disabled: HC_DARK_DISABLED,
            on_accent: BLACK,
            on_inverse: BLACK,
            accent: HC_DARK_ACCENT,
            accent_hover: HC_DARK_ACCENT_HOVER,
            accent_pressed: HC_DARK_ACCENT_HOVER,
            accent_subtle: HC_DARK_ACCENT_SUBTLE,
            border_subtle: WHITE,
            border_default: WHITE,
            border_control: WHITE,
            hover: HC_DARK_HOVER,
            pressed: HC_DARK_PRESSED,
            error: HC_DARK_ERROR,
            error_subtle: BLACK,
            warning: HC_DARK_WARNING,
            warning_icon: HC_DARK_WARNING,
            warning_subtle: BLACK,
            success: HC_DARK_SUCCESS,
            success_subtle: BLACK,
            surround: HC_DARK_SURROUND,
            backdrop: BLACK.alpha(102),
            elevation_1: Elevation::NONE,
            elevation_2: Elevation::NONE,
            raised_border: 1.0,
        },
        Theme::HighContrastLight => Palette {
            chrome: WHITE,
            panel: WHITE,
            raised: WHITE,
            sunken: WHITE,
            inverse: BLACK,
            primary: BLACK,
            secondary: HC_LIGHT_SECONDARY,
            tertiary: HC_LIGHT_SECONDARY,
            disabled: HC_LIGHT_DISABLED,
            on_accent: WHITE,
            on_inverse: WHITE,
            accent: HC_LIGHT_ACCENT,
            accent_hover: HC_LIGHT_ACCENT_HOVER,
            accent_pressed: HC_LIGHT_ACCENT_HOVER,
            accent_subtle: HC_LIGHT_ACCENT_SUBTLE,
            border_subtle: BLACK,
            border_default: BLACK,
            border_control: BLACK,
            hover: HC_LIGHT_HOVER,
            pressed: HC_LIGHT_PRESSED,
            error: HC_LIGHT_ERROR,
            error_subtle: WHITE,
            warning: HC_LIGHT_WARNING,
            warning_icon: HC_LIGHT_WARNING,
            warning_subtle: WHITE,
            success: HC_LIGHT_SUCCESS,
            success_subtle: WHITE,
            surround: HC_LIGHT_SURROUND,
            backdrop: SLATE_900.alpha(61),
            elevation_1: Elevation::NONE,
            elevation_2: Elevation::NONE,
            raised_border: 1.0,
        },
    }
}

/// The tokens of `theme` at `density`. Deterministic: the same arguments give the same value.
pub fn tokens(theme: Theme, density: Density) -> Tokens {
    use ramp::*;
    let p = palette(theme);
    let compact = density == Density::Compact;
    Tokens {
        theme,
        density,
        surface_chrome: p.chrome,
        surface_panel: p.panel,
        surface_raised: p.raised,
        surface_sunken: p.sunken,
        surface_inverse: p.inverse,
        text_primary: p.primary,
        text_secondary: p.secondary,
        text_tertiary: p.tertiary,
        text_disabled: p.disabled,
        text_on_accent: p.on_accent,
        text_on_inverse: p.on_inverse,
        accent: p.accent,
        accent_hover: p.accent_hover,
        accent_pressed: p.accent_pressed,
        accent_subtle: p.accent_subtle,
        border_subtle: p.border_subtle,
        border_default: p.border_default,
        border_control: p.border_control,
        focus: p.accent,
        state_hover: p.hover,
        state_pressed: p.pressed,
        error: p.error,
        error_subtle: p.error_subtle,
        warning: p.warning,
        warning_icon: p.warning_icon,
        warning_subtle: p.warning_subtle,
        success: p.success,
        success_subtle: p.success_subtle,
        info: p.accent,
        info_subtle: p.accent_subtle,
        // Light: slate-900 at 24 % (§6.12); dark: black at 40 % (FT-557). Opaque-free in high contrast is not required
        // for a backdrop.
        backdrop: p.backdrop,
        canvas_surround: p.surround,
        canvas_sheet: WHITE,
        canvas_sheet_line: BLACK,
        canvas_grid_dot: GRID_DOT,
        canvas_selection: VOLT_600,
        canvas_hover: VOLT_600,
        // volt-600 at 60 %.
        canvas_ghost: VOLT_600.alpha(153),
        canvas_snap: VOLT_600,
        canvas_highlight: [VERMILLION, BLUISH_GREEN, REDDISH_PURPLE, DARK_ORANGE],
        // `color.text.primary`'s light value in every theme (the sheet is white).
        canvas_measure: SLATE_900,
        // slate-400 at 30 %.
        canvas_stale: SLATE_400.alpha(77),
        // `color.surface.inverse` at 94 %.
        canvas_chip: p.inverse.alpha(240),
        canvas_chip_text: p.on_inverse,
        canvas_highlight_label: BLACK,
        elevation_1: p.elevation_1,
        elevation_2: p.elevation_2,
        border_raised_width: p.raised_border,
        type_caption: TypeToken::regular(11.0, 14.0),
        type_small: TypeToken::regular(12.0, 16.0),
        type_label: TypeToken {
            size: 12.0,
            line: 16.0,
            semibold: true,
            upper: true,
        },
        type_body: TypeToken::regular(13.0, 18.0),
        type_body_strong: TypeToken::semibold(13.0, 18.0),
        type_title: TypeToken::semibold(15.0, 20.0),
        type_display: TypeToken::semibold(20.0, 26.0),
        type_mono: TypeToken::regular(12.0, 16.0),
        space_1: 2.0,
        space_2: 4.0,
        space_icon: 6.0,
        space_3: 8.0,
        space_4: 12.0,
        space_5: 16.0,
        space_6: 24.0,
        space_7: 32.0,
        size_row: if compact { 24.0 } else { 28.0 },
        size_control: 28.0,
        size_control_inline: 24.0,
        size_tab_row: 32.0,
        size_command_row: if compact { 36.0 } else { 52.0 },
        size_command_large_height: if compact { 28.0 } else { 46.0 },
        size_command_large_min_width: if compact { 0.0 } else { 52.0 },
        size_command_small: 24.0,
        size_quick_access_width: 28.0,
        size_quick_access_height: 24.0,
        size_search_width: 280.0,
        size_search_height: 24.0,
        size_panel_header: 32.0,
        size_dock_tab_bar: 32.0,
        size_rail: 40.0,
        size_rail_button: 32.0,
        size_status: 26.0,
        size_status_field: 22.0,
        size_panel_left: 280.0,
        size_panel_right: 280.0,
        size_dock_bottom: 184.0,
        size_dock_bottom_min: 300.0,
        size_property_label: 96.0,
        size_tree_indent: 20.0,
        size_table_header: 24.0,
        size_badge: 18.0,
        size_chip: 16.0,
        size_button_min_width: 72.0,
        size_dialog_small: 440.0,
        size_dialog_form: 640.0,
        size_palette: 560.0,
        size_palette_row: 32.0,
        size_toast: 300.0,
        size_tooltip_max: 320.0,
        size_table_column: 110.0,
        size_table_column_min: 40.0,
        size_table_column_max: 480.0,
        size_severity_icon: 16.0,
        size_chooser_min_width: 240.0,
        size_chooser_max_width: 360.0,
        size_chooser_rows: 8,
        size_preview_list_rows: 10,
        size_card_strip: 28.0,
        table_frozen_edge: p.border_default,
        table_cursor: p.accent,
        table_cursor_idle: p.accent,
        history_undone: p.tertiary,
        card_strip: p.sunken,
        radius_none: 0.0,
        radius_mark: 2.0,
        radius_sm: 4.0,
        radius_md: 6.0,
        radius_lg: 8.0,
        radius_pill: 999.0,
        border_width: 1.0,
        focus_width: 2.0,
        icon_sm: 16.0,
        icon_md: 20.0,
        icon_lg: 24.0,
        icon_stroke: 1.5,
        icon_stroke_lg: 1.75,
        motion_fast: 80,
        motion_base: 140,
    }
}
