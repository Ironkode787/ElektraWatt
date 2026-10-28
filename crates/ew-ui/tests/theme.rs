// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The token module of WP-S2.17 part 1 (§12.1): DL1 the contrast lint of NFR-A11Y-04, DL2 the blessed dump
//! (`tests/expected/tokens.txt`, rewritten by `EW_BLESS=1`), DL3 the mapping onto egui, DL4 the literal guard, DL5
//! the SemiBold face, DL8 the token values of the approved mockup (read with `include_str!`).
//!
//! ```text
//! EW_BLESS=1 cargo nextest run -p ew-ui --test theme
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{FontFamily, TextStyle};
use ew_ui::theme::{
    self, Density, PairKind, Rgba, Theme, TokenValue, Tokens, apply, contrast, style_of, tokens,
};

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/tokens.txt");
const MOCKUP: &str = include_str!("../../../docs/design/assets/design-language-mockup.html");

fn all() -> Vec<Tokens> {
    let mut out = Vec::new();
    for theme in Theme::ALL {
        for density in Density::ALL {
            out.push(tokens(theme, density));
        }
    }
    out
}

/// Whether the test rewrites its expected file (`EW_BLESS=1`).
// Program boundary of the test binary: the bless switch; the dump does not depend on it.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

// ---------------------------------------------------------------------------------------------------------------
// DL1

#[test]
fn dl1_contrast_is_wcag_2_1() {
    let white = Rgba::hex(0xFFFFFF);
    assert!((contrast(Rgba::hex(0x0072B2), white) - 5.19).abs() <= 0.01);
    assert!((contrast(Rgba::hex(0x767676), white) - 4.54).abs() <= 0.01);
    assert!((contrast(white, Rgba::hex(0x000000)) - 21.0).abs() <= 0.01);
    assert_eq!(contrast(white, white), 1.0);
    // Symmetric.
    assert_eq!(
        contrast(white, Rgba::hex(0x0B63CE)),
        contrast(Rgba::hex(0x0B63CE), white)
    );
}

#[test]
fn dl1_every_declared_pair_of_every_theme_meets_its_floor() {
    let mut failures = Vec::new();
    for t in all() {
        for pair in t.pairs() {
            let fg = t
                .colour(pair.fg)
                .unwrap_or_else(|| panic!("{} has no colour", pair.fg));
            let bg = t
                .colour(pair.bg)
                .unwrap_or_else(|| panic!("{} has no colour", pair.bg));
            assert_eq!(fg.a, 255, "{} is opaque", pair.fg);
            assert_eq!(bg.a, 255, "{} is opaque", pair.bg);
            let ratio = contrast(fg, bg);
            if ratio < pair.floor(t.theme) {
                failures.push(format!(
                    "{} {}: {} on {} is {ratio:.2}, needs {}",
                    t.theme.word(),
                    t.density.word(),
                    pair.fg,
                    pair.bg,
                    pair.floor(t.theme)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn dl1_the_ratios_of_doc_16a_section_4_5_are_recomputed() {
    let light = tokens(Theme::Light, Density::Comfortable);
    let dark = tokens(Theme::Dark, Density::Comfortable);
    let r = |t: &Tokens, a: &str, b: &str| {
        (contrast(t.colour(a).unwrap(), t.colour(b).unwrap()) * 100.0).round() / 100.0
    };
    // Rounded as 16a prints them.
    assert_eq!(r(&light, "color.accent", "color.surface.panel"), 5.69);
    assert_eq!(r(&light, "color.text.secondary", "color.surface.panel"), 6.75);
    assert_eq!(r(&light, "color.text.tertiary", "color.surface.chrome"), 4.86);
    assert_eq!(r(&light, "color.border.control", "color.surface.panel"), 3.31);
    assert_eq!(r(&dark, "color.text.on_accent", "color.accent"), 6.65);
    // Canvas slots on white (§4.6).
    assert_eq!(r(&light, "canvas.highlight.1", "canvas.sheet"), 3.87);
    assert_eq!(r(&light, "canvas.highlight.3", "canvas.sheet"), 3.06);
    // The high-contrast themes add the boundary rule and the 7:1 text floor.
    let hc = tokens(Theme::HighContrastDark, Density::Comfortable);
    assert!(hc.pairs().iter().any(|p| p.fg == "color.border.default"));
    let text = hc.pairs().into_iter().find(|p| p.kind == PairKind::Text).unwrap();
    assert_eq!(text.floor(Theme::HighContrastDark), 7.0);
}

#[test]
fn dl1_high_contrast_themes_have_no_translucency_and_no_shadow() {
    for theme in [Theme::HighContrastDark, Theme::HighContrastLight] {
        let t = tokens(theme, Density::Comfortable);
        for (name, value) in t.entries() {
            match value {
                TokenValue::Colour(c) if name.starts_with("color.") && name != "color.backdrop" => {
                    assert_eq!(c.a, 255, "{} {name} is opaque", theme.word());
                }
                TokenValue::Shadow(e) => {
                    assert!(
                        e.layers.iter().all(|l| l.colour.a == 0),
                        "{} {name}",
                        theme.word()
                    );
                }
                _ => {}
            }
        }
        assert_eq!(t.border_raised_width, 1.0, "borders replace shadows");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DL2

fn dump() -> String {
    let mut out = String::from(
        "# The design tokens of doc 16a §4 (WP-S2.17 part 1): every token of every theme and density, a colour pair's\n\
         # contrast after the colour (\"on <background> <ratio>\"). Rewritten by EW_BLESS=1; a change is a reviewed diff.\n",
    );
    for t in all() {
        let pairs = t.pairs();
        out.push_str(&format!("\n[{} {}]\n", t.theme.word(), t.density.word()));
        for (name, value) in t.entries() {
            out.push_str(name);
            out.push(' ');
            out.push_str(&value.text());
            for p in pairs.iter().filter(|p| p.fg == name) {
                let ratio = contrast(t.colour(p.fg).unwrap(), t.colour(p.bg).unwrap());
                out.push_str(&format!(" on {} {ratio:.2}", p.bg));
            }
            out.push('\n');
        }
    }
    out
}

#[test]
fn dl2_every_alias_exists_in_every_theme_and_density() {
    let names = |t: &Tokens| t.entries().into_iter().map(|(n, _)| n).collect::<Vec<_>>();
    let reference = names(&tokens(Theme::Light, Density::Comfortable));
    let mut unique = reference.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), reference.len(), "no name twice");
    for t in all() {
        assert_eq!(names(&t), reference, "{} {}", t.theme.word(), t.density.word());
    }
    // Every pair names tokens that exist.
    for p in theme::PAIRS.iter().chain(theme::HC_PAIRS) {
        assert!(reference.contains(&p.fg) && reference.contains(&p.bg), "{p:?}");
    }
}

#[test]
fn dl2_the_dump_equals_the_blessed_file_and_is_deterministic() {
    let a = dump();
    assert_eq!(a, dump(), "two runs byte-identical");
    if bless() {
        std::fs::create_dir_all(Path::new(EXPECTED).parent().unwrap()).unwrap();
        std::fs::write(EXPECTED, &a).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(EXPECTED)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        expected == a,
        "tests/expected/tokens.txt differs; re-bless with EW_BLESS=1 cargo nextest run -p ew-ui --test theme"
    );
}

#[test]
fn dl2_density_changes_sizes_only() {
    for theme in Theme::ALL {
        let c = tokens(theme, Density::Comfortable);
        let k = tokens(theme, Density::Compact);
        let changed: Vec<&str> = c
            .entries()
            .into_iter()
            .zip(k.entries())
            .filter(|(a, b)| a.1 != b.1)
            .map(|(a, _)| a.0)
            .collect();
        assert_eq!(
            changed,
            [
                "size.row",
                "size.command_row",
                "size.command.large.height",
                "size.command.large.min_width"
            ]
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DL3

#[test]
fn dl3_apply_maps_the_tokens_onto_egui() {
    for t in all() {
        let ctx = egui::Context::default();
        apply(&t, &ctx);
        let style = ctx.global_style();
        assert_eq!(style.visuals, style_of(&t).visuals);
        let v = &style.visuals;
        let what = format!("{} {}", t.theme.word(), t.density.word());
        assert_eq!(v.dark_mode, t.theme.is_dark(), "{what}");
        assert_eq!(v.panel_fill, t.surface_panel.color32(), "{what}");
        assert_eq!(v.window_fill, t.surface_raised.color32(), "{what}");
        assert_eq!(v.selection.bg_fill, t.accent_subtle.color32(), "{what}");
        assert_eq!(v.selection.stroke.color, t.accent.color32(), "{what}");
        assert_eq!(v.hyperlink_color, t.accent.color32(), "{what}");
        assert_eq!(v.error_fg_color, t.error.color32(), "{what}");
        assert_eq!(v.warn_fg_color, t.warning.color32(), "{what}");
        assert_eq!(v.window_corner_radius, theme::radius(t.radius_lg), "{what}");
        assert_eq!(v.menu_corner_radius, theme::radius(t.radius_md), "{what}");
        for w in [
            &v.widgets.noninteractive,
            &v.widgets.inactive,
            &v.widgets.hovered,
            &v.widgets.active,
            &v.widgets.open,
        ] {
            assert_eq!(w.corner_radius, theme::radius(t.radius_md), "{what}");
            assert_eq!(w.expansion, 0.0, "{what}: nothing grows under the pointer");
        }
        assert_eq!(
            v.widgets.inactive.bg_stroke.color,
            t.border_control.color32(),
            "{what}"
        );
        assert_eq!(style.spacing.interact_size.y, t.size_control, "{what}");
        assert_eq!(style.spacing.indent, t.size_tree_indent, "{what}");
        let font = |s: TextStyle| style.text_styles[&s].clone();
        assert_eq!(font(TextStyle::Small).size, t.type_caption.size);
        assert_eq!(font(TextStyle::Body).size, t.type_body.size);
        assert_eq!(font(TextStyle::Button).size, t.type_body.size);
        assert_eq!(font(TextStyle::Heading), theme::font(&t.type_title));
        assert_eq!(font(TextStyle::Monospace).size, t.type_mono.size);
        assert_eq!(font(TextStyle::Name("small".into())).size, t.type_small.size);
        assert_eq!(
            font(TextStyle::Name("strong".into())),
            theme::font(&t.type_body_strong)
        );
        assert_eq!(
            font(TextStyle::Name("display".into())),
            theme::font(&t.type_display)
        );
        assert_eq!(style.animation_time, 0.08);
        assert_eq!(
            *theme::current(&ctx),
            t,
            "{what}: the widget code reads the same tokens"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// DL5

#[test]
fn dl5_a_strong_label_is_laid_out_with_the_semibold_face() {
    let ctx = egui::Context::default();
    ew_ui::app::setup(&ctx);
    let t = tokens(Theme::Light, Density::Comfortable);
    apply(&t, &ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
    out.textures_delta.clear();
    let strong = ctx.global_style().text_styles[&TextStyle::Name("strong".into())].clone();
    assert_eq!(
        strong.family,
        FontFamily::Name(ew_ui::fonts::SEMIBOLD_FAMILY.into())
    );
    let width = |font: egui::FontId| {
        ctx.fonts_mut(|f| {
            f.layout_no_wrap("Contactor -K1".to_owned(), font, egui::Color32::BLACK)
                .size()
                .x
        })
    };
    let regular = width(egui::FontId::proportional(strong.size));
    let semibold = width(strong);
    assert!(
        semibold > regular,
        "SemiBold is wider ({semibold} against {regular})"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// DL4: the literal guard

/// The literal styling of one source text: `(line, what)` per finding. Comments are skipped.
fn literals(source: &str) -> Vec<(usize, String)> {
    let mut code = String::with_capacity(source.len());
    let mut lines = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let without = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        lines.push((code.len(), n + 1));
        code.push_str(without);
        code.push('\n');
    }
    let line_of = |at: usize| {
        lines
            .iter()
            .rev()
            .find(|(start, _)| *start <= at)
            .map_or(0, |l| l.1)
    };
    let mut out = Vec::new();
    let mut find = |needle: &str, what: &str, needs_number: bool| {
        let mut from = 0;
        while let Some(i) = code[from..].find(needle) {
            let at = from + i;
            from = at + needle.len();
            if needs_number {
                let rest = code[from..].trim_start();
                if !rest.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
                    continue;
                }
            }
            out.push((line_of(at), what.to_owned()));
        }
    };
    for ctor in [
        "Color32::from_rgb(",
        "Color32::from_rgba",
        "Color32::from_hex",
        "Color32::from_gray",
        "Color32::from_black_alpha",
        "Color32::from_white_alpha",
        "Rgba::hex(",
    ] {
        find(ctor, ctor, false);
    }
    for (ctor, what) in [
        ("CornerRadius::same(", "numeric CornerRadius"),
        ("Stroke::new(", "numeric Stroke"),
        ("Margin::same(", "numeric Margin"),
        ("Margin::symmetric(", "numeric Margin"),
        ("FontId::proportional(", "numeric font size"),
        ("FontId::monospace(", "numeric font size"),
        ("FontId::new(", "numeric font size"),
    ] {
        find(ctor, what, true);
    }
    // Named colour constants: `Color32::WHITE`, `Color32::TRANSPARENT`, …
    let mut from = 0;
    while let Some(i) = code[from..].find("Color32::") {
        let at = from + i;
        from = at + "Color32::".len();
        if code[from..].starts_with(|c: char| c.is_ascii_uppercase()) {
            out.push((line_of(at), "named Color32 constant".to_owned()));
        }
    }
    // A painter's rectangle with a numeric corner radius: the second argument of `rect_filled(` / `rect_stroke(`.
    for call in ["rect_filled(", "rect_stroke("] {
        let mut from = 0;
        while let Some(i) = code[from..].find(call) {
            let at = from + i;
            from = at + call.len();
            if let Some(arg) = argument(&code[from..], 1)
                && arg.trim().starts_with(|c: char| c.is_ascii_digit())
            {
                out.push((line_of(at), format!("numeric radius in {call}")));
            }
        }
    }
    // Hex colour literals.
    for (n, line) in code.lines().enumerate() {
        if let Some(i) = line.find("\"#")
            && line[i + 2..]
                .chars()
                .take(6)
                .filter(char::is_ascii_hexdigit)
                .count()
                == 6
        {
            out.push((n + 1, "hex colour literal".to_owned()));
        }
    }
    out.sort();
    out
}

/// The `index`th top-level argument of a call whose text starts after its `(`.
fn argument(text: &str, index: usize) -> Option<&str> {
    let mut depth = 0usize;
    let mut start = 0;
    let mut n = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => {
                return (n == index).then(|| &text[start..i]);
            }
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if n == index {
                    return Some(&text[start..i]);
                }
                n += 1;
                start = i + 1;
            }
            _ => {}
        }
    }
    None
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn dl4_no_literal_colour_size_or_radius_outside_the_theme_module() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    let mut found = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel.starts_with("theme/") || rel == "icons/generated.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        for (line, what) in literals(&text) {
            found.push(format!("src/{rel}:{line}: {what}"));
        }
    }
    assert!(
        found.is_empty(),
        "literal styling outside theme/ (D4):\n{}",
        found.join("\n")
    );
}

#[test]
fn dl4_the_guard_finds_each_kind_of_literal() {
    let bad = r##"
        let a = Color32::from_rgb(1, 2, 3);
        let b = egui::Color32::from_rgba_unmultiplied(1, 2, 3, 4);
        let c = Color32::WHITE;
        let d = Stroke::new(1.5, colour);
        let e = CornerRadius::same(4);
        let f = Margin::symmetric(8, 2);
        painter.rect_filled(rect, 2.0, fill);
        painter.rect_stroke(
            rect,
            0.0,
            stroke,
            kind,
        );
        let g = FontId::proportional(12.0);
        let h = "#0B63CE";
    "##;
    let found = literals(bad);
    assert_eq!(found.len(), 10, "{found:#?}");
    let good = r##"
        // Color32::from_rgb in a comment is fine.
        let a = t.accent.color32();
        let d = Stroke::new(t.border_width, colour);
        painter.rect_filled(rect, t.radius_sm, fill);
        let g = theme::font(&t.type_small);
        let i = ColorImage::from_rgba_unmultiplied(size, &pixels);
    "##;
    assert_eq!(literals(good), Vec::new());
}

// ---------------------------------------------------------------------------------------------------------------
// DL8: the approved mockup's tokens

/// The custom properties of the CSS block that starts with `selector {`.
fn block(selector: &str) -> BTreeMap<String, String> {
    let start = MOCKUP
        .find(&format!("{selector} {{"))
        .unwrap_or_else(|| panic!("{selector} block"));
    let body = &MOCKUP[start + selector.len() + 2..];
    let body = &body[..body.find('}').unwrap()];
    let mut out = BTreeMap::new();
    for decl in body.split(';') {
        let decl = decl.trim();
        if let Some(rest) = decl.strip_prefix("--")
            && let Some((name, value)) = rest.split_once(':')
        {
            out.insert(name.trim().to_owned(), value.trim().to_owned());
        }
    }
    out
}

/// The CSS rule's `property` value for `selector` (the first rule that starts with it).
fn rule(selector: &str, property: &str) -> String {
    let start = MOCKUP
        .find(&format!("\n{selector} {{"))
        .unwrap_or_else(|| panic!("rule {selector}"));
    let body = &MOCKUP[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find('}').unwrap()];
    body.split(';')
        .filter_map(|d| d.split_once(':'))
        .find(|(k, _)| k.trim() == property)
        .map(|(_, v)| v.trim().to_owned())
        .unwrap_or_else(|| panic!("{selector} {property}"))
}

/// A CSS colour: `#RRGGBB` or `rgba(r,g,b,a)`, alpha as a fraction.
fn css_colour(text: &str) -> (u8, u8, u8, f64) {
    if let Some(hex) = text.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).unwrap();
        return ((v >> 16) as u8, (v >> 8) as u8, v as u8, 1.0);
    }
    let inner = text
        .strip_prefix("rgba(")
        .and_then(|t| t.strip_suffix(')'))
        .unwrap();
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    (
        parts[0].parse().unwrap(),
        parts[1].parse().unwrap(),
        parts[2].parse().unwrap(),
        parts[3].parse().unwrap(),
    )
}

fn px(text: &str) -> f32 {
    text.trim_end_matches("px").parse().unwrap()
}

fn assert_colour(what: &str, css: &str, token: Rgba) {
    let (r, g, b, a) = css_colour(css);
    assert_eq!(
        (token.r, token.g, token.b),
        (r, g, b),
        "{what}: {css} against {}",
        token.text()
    );
    assert!(
        (f64::from(token.a) - a * 255.0).abs() <= 0.5 + 1e-9,
        "{what}: alpha {a} against {}",
        token.a
    );
}

/// `x y blur [spread] colour` layers, split at the top-level commas.
fn css_shadow(text: &str) -> Vec<(i8, i8, u8, u8, String)> {
    let mut layers = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    for part in parts {
        let part = part.trim();
        let colour_at = part.find("rgba").unwrap();
        let lengths: Vec<f32> = part[..colour_at].split_whitespace().map(px).collect();
        let spread = lengths.get(3).copied().unwrap_or(0.0);
        layers.push((
            lengths[0] as i8,
            lengths[1] as i8,
            lengths[2] as u8,
            spread as u8,
            part[colour_at..].to_owned(),
        ));
    }
    layers
}

/// The mockup's custom property → the token's dump name (WP-S2.17 §12.0).
fn token_name(property: &str) -> Option<String> {
    Some(match property {
        "font" => return None,
        "hover" => "color.state.hover".to_owned(),
        "pressed" => "color.state.pressed".to_owned(),
        "canvas-surround" => "canvas.surround".to_owned(),
        "sheet" => "canvas.sheet".to_owned(),
        "sheet-line" => "canvas.sheet_line".to_owned(),
        "grid-dot" => "canvas.grid_dot".to_owned(),
        "selection" => "canvas.selection".to_owned(),
        "hl1" => "canvas.highlight.1".to_owned(),
        "hl2" => "canvas.highlight.2".to_owned(),
        "shadow-1" => "elevation.1".to_owned(),
        "shadow-2" => "elevation.2".to_owned(),
        "row" => "size.row".to_owned(),
        "bar-cmd" => "size.command_row".to_owned(),
        p if p.starts_with("surface-")
            || p.starts_with("text-")
            || p.starts_with("accent")
            || p.starts_with("border-")
            || p.starts_with("error")
            || p.starts_with("warning")
            || p.starts_with("success") =>
        {
            // `--text-on-accent` → `color.text.on_accent`; `--surface-chrome` → `color.surface.chrome`.
            let (head, tail) = p.split_once('-').unwrap_or((p, ""));
            if tail.is_empty() {
                format!("color.{head}")
            } else {
                format!("color.{head}.{}", tail.replace('-', "_"))
            }
        }
        other => panic!("the mockup's --{other} has no token in the map of §12.0"),
    })
}

fn check_block(t: &Tokens, properties: &BTreeMap<String, String>) -> usize {
    let mut checked = 0;
    for (property, css) in properties {
        let Some(name) = token_name(property) else {
            continue;
        };
        let value = t
            .get(&name)
            .unwrap_or_else(|| panic!("--{property} maps to {name}, which is no token"));
        let what = format!("{} {}: --{property} ({name})", t.theme.word(), t.density.word());
        match value {
            TokenValue::Colour(c) => assert_colour(&what, css, c),
            TokenValue::Px(v) => assert_eq!(v, px(css), "{what}"),
            TokenValue::Shadow(e) => {
                let layers = css_shadow(css);
                assert_eq!(layers.len(), 2, "{what}");
                for (layer, (x, y, blur, spread, colour)) in e.layers.iter().zip(layers) {
                    assert_eq!(
                        (layer.x, layer.y, layer.blur, layer.spread),
                        (x, y, blur, spread),
                        "{what}"
                    );
                    assert_colour(&what, &colour, layer.colour);
                }
            }
            other => panic!("{what}: unexpected {other:?}"),
        }
        checked += 1;
    }
    checked
}

#[test]
fn dl8_the_tokens_equal_the_mockups_custom_properties() {
    let root = block(":root");
    let mut dark = root.clone();
    dark.extend(block("[data-theme=dark]"));
    let mut compact = root.clone();
    compact.extend(block("[data-density=compact]"));
    let n = check_block(&tokens(Theme::Light, Density::Comfortable), &root);
    assert_eq!(
        n,
        root.len() - 1,
        "every property of :root but the font is a token"
    );
    check_block(&tokens(Theme::Dark, Density::Comfortable), &dark);
    check_block(&tokens(Theme::Light, Density::Compact), &compact);
    check_block(&tokens(Theme::Dark, Density::Compact), &{
        let mut d = dark.clone();
        d.extend(block("[data-density=compact]"));
        d
    });
}

#[test]
fn dl8_g10_and_g15_equal_the_mockups_geometry() {
    let light = tokens(Theme::Light, Density::Comfortable);
    let compact = tokens(Theme::Light, Density::Compact);
    // G10: row height (via --row) and the property label column.
    assert_eq!(light.size_row, 28.0);
    assert_eq!(compact.size_row, 24.0);
    let columns = rule(".prow", "grid-template-columns");
    assert_eq!(
        px(columns.split_whitespace().next().unwrap()),
        light.size_property_label
    );
    // G15: radii.
    let r = |selector: &str| px(&rule(selector, "border-radius"));
    for selector in [".qa", ".ghost"] {
        assert_eq!(r(selector), light.radius_sm, "icon buttons: {selector}");
    }
    assert_eq!(
        px(&rule(".prow .val", "border-radius")),
        light.radius_sm,
        "value cells"
    );
    for selector in [".cmd", ".rbtn", ".primary", ".search", ".filter", ".zoom"] {
        assert_eq!(r(selector), light.radius_md, "{selector}");
    }
    assert_eq!(r(".toast"), light.radius_lg, "toasts");
    for selector in [".hud", ".badge"] {
        assert_eq!(r(selector), light.radius_pill, "{selector}");
    }
    // The other sizes the mockup's CSS states.
    assert_eq!(px(&rule(".tabrow", "height")), light.size_tab_row);
    assert_eq!(px(&rule(".search", "width")), light.size_search_width);
    assert_eq!(px(&rule(".rbtn", "width")), light.size_rail_button);
    assert_eq!(px(&rule(".phead", "height")), light.size_panel_header);
    assert_eq!(px(&rule(".sfield", "height")), light.size_status_field);
    assert_eq!(px(&rule(".toast", "width")), light.size_toast);
    assert_eq!(px(&rule(".bottom", "height")), light.size_dock_bottom);
    assert_eq!(px(&rule(".cmd", "height")), light.size_command_large_height);
    assert_eq!(px(&rule(".cmd", "min-width")), light.size_command_large_min_width);
}
