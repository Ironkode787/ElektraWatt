// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The icon set (WP-S2.17 part 6, §12.6 IC2 to IC4 and IC6; doc 16a §4.11). IC1 and the generator half of IC2
//! are the `xtask` tests of `gen-icons`; IC5 (the gallery snapshot) needs part 1's component gallery and
//! snapshot renderer.

use egui::epaint::{Mesh, TessellationOptions, Tessellator};
use egui::{Color32, Pos2, Rect, Shape, Vec2};
use ew_ui::icons::{self, COUNT, IconId, KEYS};

/// The keys doc 16a names for the built widgets (§5.5, §6.21, §6.23, §6.24) and the project's glyphs (§4.11).
const DOC_16A_KEYS: &[&str] = &[
    "sparkles",
    "table",
    "arrow-left",
    "arrow-right",
    "arrow-narrow-up",
    "arrow-narrow-down",
    "math-function",
    "eye",
    "circle-x",
    "alert-triangle",
    "info-circle",
    "arrow-back-up",
    "lock",
    "history",
    "list-details",
    "file",
    "ew-cable",
    "ew-terminal-strip",
    "ew-interruption-point",
    "ew-black-box",
    "ew-potential",
    "ew-structure-box",
    "ew-qet-grid",
];

/// The mockup's icon controls by their `title` (up to " (" or ":"), and the key each maps to. The mockup draws
/// its icons inline without Tabler names, so this table is the mapping; the test below fails when the mockup
/// gains an icon control that is not in it.
const MOCKUP_TITLES: &[(&str, &str)] = &[
    ("Align", "layout-align-left"),
    ("Black box", "ew-black-box"),
    ("Cables", "ew-cable"),
    ("Close", "x"),
    ("Derived; read-only", "math-function"),
    ("Device from part", "package"),
    ("Dismiss", "x"),
    ("Findings", "circle-x"),
    ("Fit page", "maximize"),
    ("History", "history"),
    ("Interruption point", "ew-interruption-point"),
    ("Measure", "ruler-measure"),
    // The app button: the product mark's bolt and the chevron.
    ("Menu", "chevron-down"),
    ("Mirror", "flip-vertical"),
    ("More", "dots"),
    ("Pages and devices", "files"),
    ("Parts", "package"),
    ("Potential", "ew-potential"),
    ("Potentials", "ew-potential"),
    ("Redo", "arrow-forward-up"),
    ("Renumber devices", "sort-ascending-numbers"),
    ("Rotate", "rotate-clockwise"),
    ("Run the check scheme", "list-check"),
    ("Save", "device-floppy"),
    ("Select", "pointer"),
    ("Sort by Terminal", "arrow-narrow-up"),
    ("Structure", "sitemap"),
    ("Symbol", "circuit-switch-open"),
    ("Symbols", "category"),
    ("Table", "table"),
    ("Terminal", "circle-dot"),
    ("Terminals", "ew-terminal-strip"),
    ("Text", "typography"),
    ("Undo", "arrow-back-up"),
    ("Wire", "route-2"),
];

/// The mockup's icons without a title (the app button's chevron, the search field, filters, tree kinds, the
/// severities, the saved state, the Columns menu, the chooser's kinds).
const MOCKUP_UNTITLED: &[&str] = &[
    "chevron-down",
    "search",
    "filter",
    "folder",
    "file",
    "circle-x",
    "alert-triangle",
    "check",
    "circle-dot",
    "route-2",
    "ew-black-box",
];

const MOCKUP: &str = include_str!("../../../docs/design/assets/design-language-mockup.html");

fn key(k: &str) -> IconId {
    IconId::from_key(k).unwrap_or_else(|| panic!("the icon table has no `{k}`"))
}

#[test]
fn keys_round_trip_and_are_sorted() {
    assert_eq!(KEYS.len(), COUNT);
    assert_eq!(IconId::ALL.len(), COUNT);
    assert!(KEYS.windows(2).all(|w| w[0] < w[1]), "keys sorted and unique");
    for id in IconId::ALL {
        assert_eq!(IconId::from_key(id.key()), Some(id));
        assert_eq!(
            format!("{id:?}"),
            id.key().replace('-', "_"),
            "snake_case variant"
        );
    }
    assert_eq!(IconId::from_key("no-such-icon"), None);
}

/// IC2: every key the design language names exists in the table.
#[test]
fn ic2_every_named_key_exists() {
    for k in DOC_16A_KEYS {
        key(k);
    }
}

/// IC6 (names): every icon control of the mockup is mapped, and every mapped key exists.
#[test]
fn ic6_every_mockup_icon_is_in_the_table() {
    let mut rest = MOCKUP;
    let mut seen = 0;
    while let Some(at) = rest.find("title=\"") {
        rest = &rest[at + 7..];
        let end = rest.find('"').expect("closed title");
        let title = &rest[..end];
        // The element's own content: up to the first closing tag.
        let scope_end = rest.find("</").unwrap_or(rest.len());
        if !rest[..scope_end].contains("<svg class=\"i") {
            continue;
        }
        let short = title
            .split(" (")
            .next()
            .and_then(|t| t.split(':').next())
            .unwrap_or(title);
        let mapped = MOCKUP_TITLES
            .iter()
            .find(|(t, _)| *t == short)
            .unwrap_or_else(|| panic!("the mockup's icon control `{title}` is not mapped"));
        key(mapped.1);
        seen += 1;
    }
    assert!(seen >= 30, "found {seen} icon controls in the mockup");
    for (_, k) in MOCKUP_TITLES {
        key(k);
    }
    for k in MOCKUP_UNTITLED {
        key(k);
    }
}

fn mesh(shapes: Vec<Shape>, ppp: f32) -> Mesh {
    let mut t = Tessellator::new(ppp, TessellationOptions::default(), [1, 1], Vec::new());
    let mut mesh = Mesh::default();
    for s in shapes {
        t.tessellate_shape(s, &mut mesh);
    }
    mesh
}

/// IC3: every icon paints at 16, 20 and 24 px at ppp 1 and 2 without panic and within its rect (plus the
/// anti-aliasing feather of one physical pixel).
#[test]
fn ic3_every_icon_paints_within_its_rect() {
    let color = Color32::from_rgb(0x3A, 0x43, 0x50);
    for (size, stroke) in [(16.0, 1.5), (20.0, 1.5), (24.0, 1.75)] {
        let rect = Rect::from_min_size(Pos2::new(10.0, 30.0), Vec2::splat(size));
        for ppp in [1.0_f32, 2.0] {
            let allowed = rect.expand(1.0 / ppp + 1e-3);
            for id in IconId::ALL {
                let shapes = icons::shapes(id, rect, color, stroke);
                assert!(!shapes.is_empty(), "{id:?} paints nothing");
                let m = mesh(shapes, ppp);
                assert!(!m.vertices.is_empty(), "{id:?} tessellates to nothing");
                for v in &m.vertices {
                    assert!(
                        allowed.contains(v.pos),
                        "{id:?} at {size} px, ppp {ppp}: vertex {:?} outside {rect:?}",
                        v.pos
                    );
                }
            }
        }
    }
}

/// IC6 (metrics): at 16 and 20 px with the 1.5-px stroke the paths carry exactly that stroke and the colour
/// given (secondary at rest, accent when active: the caller's role colour), and the icon fills its square.
#[test]
fn ic6_icons_paint_at_the_mockup_metrics() {
    // The role colours of the light theme (doc 16a §4.11: text.secondary at rest, accent when active).
    let light = ew_ui::theme::tokens(ew_ui::theme::Theme::Light, ew_ui::theme::Density::Comfortable);
    let secondary = light.text_secondary.color32();
    let accent = light.accent.color32();
    for size in [16.0_f32, 20.0] {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(size));
        for color in [secondary, accent] {
            let shapes = icons::shapes(key("device-floppy"), rect, color, 1.5);
            let mut paths = 0;
            for s in &shapes {
                match s {
                    Shape::Path(p) => {
                        paths += 1;
                        assert_eq!(p.stroke.width, 1.5);
                        assert_eq!(p.fill, ew_ui::theme::NONE);
                    }
                    Shape::Circle(c) => {
                        assert_eq!(c.radius, 0.75, "round caps and joins at half the stroke");
                        assert_eq!(c.fill, color);
                    }
                    other => panic!("unexpected shape {other:?}"),
                }
            }
            assert!(paths >= 3);
            // The floppy spans 4..20 of the 24 grid: 2/3 of the square.
            let bounds = shapes
                .iter()
                .fold(Rect::NOTHING, |r, s| r.union(s.visual_bounding_rect()));
            assert!(
                (bounds.width() - (size * 16.0 / 24.0 + 1.5)).abs() < 0.01,
                "{bounds:?}"
            );
        }
    }
}

/// The icon is centred in a non-square rect at the square's size.
#[test]
fn a_wide_rect_centres_the_icon() {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(48.0, 24.0));
    let shapes = icons::shapes(key("x"), rect, Color32::from_gray(0), 1.5);
    let bounds = shapes
        .iter()
        .fold(Rect::NOTHING, |r, s| r.union(s.visual_bounding_rect()));
    assert!((bounds.center().x - 24.0).abs() < 0.01);
    assert!((bounds.center().y - 12.0).abs() < 0.01);
}

/// IC4: the About dialog's third-party notices list Tabler Icons with its licence.
#[test]
fn ic4_the_notices_list_tabler_icons() {
    let notice = ew_viewmodel::about::NOTICE;
    assert!(notice.contains("Tabler Icons, version 3.50.0, licensed under the MIT License"));
    assert!(notice.contains("Copyright (c) 2020-2026"));
    assert!(notice.contains("Permission is hereby granted"));
}
