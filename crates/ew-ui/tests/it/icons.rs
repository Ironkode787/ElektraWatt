// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The icon set (IC2 to IC4 and IC6; doc 16a §4.11). IC1 and the generator half of IC2
//! are the `xtask` tests of `gen-icons`; IC5 (the gallery snapshot) needs part 1's component gallery and
//! snapshot renderer.

use egui::epaint::{Mesh, TessellationOptions, Tessellator};
use egui::{Color32, Pos2, Rect, Shape, Vec2};
use ew_ui::icons::{self, COUNT, IconId, KEYS};

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
fn every_icon_paints_within_its_rect() {
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
fn icons_paint_at_the_mockup_metrics() {
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
fn the_notices_list_tabler_icons() {
    let notice = ew_viewmodel::about::NOTICE;
    assert!(notice.contains("Tabler Icons, version 3.50.0, licensed under the MIT License"));
    assert!(notice.contains("Copyright (c) 2020-2026"));
    assert!(notice.contains("Permission is hereby granted"));
}

/// Rail, Duct, Auto-fill rail and Enclosure… each show an icon of their own: "Enclosure gets a
/// Tabler 3.50.0 icon that no other concept uses ... Auto-fill keeps `wand`", and Rail and Duct the project's
/// glyphs.
#[test]
fn the_layout_commands_show_their_own_icons() {
    let icon = |id: &str| {
        ew_viewmodel::actions::find(id)
            .and_then(|a| a.icon)
            .unwrap_or_else(|| panic!("{id} has an icon"))
    };
    assert_eq!(
        [
            icon("tool.rail_draw"),
            icon("tool.duct_draw"),
            icon("layout.auto_fill_rail"),
            icon("layout.enclosure_new"),
        ],
        ["ew-din-rail", "ew-duct", "wand", "door"]
    );
    for k in ["ew-din-rail", "ew-duct", "door"] {
        key(k);
    }
}

/// No icon key serves two concepts. A concept is a placed command of the registry or an entry of a
/// rail: a right-rail palette is the concept of the command it runs, a left-rail entry that of its pane. A key may
/// serve two entries only when they are one concept: the list below, each with its reason.
#[test]
fn no_icon_key_serves_two_concepts() {
    use std::collections::{BTreeMap, BTreeSet};
    const SHARED: &[(&str, &[&str])] = &[
        // The Interruption points navigator and the tool that places one.
        ("ew-interruption-point", &["pane ips", "tool.ip"]),
        // The Structure identifiers navigator and the command that adds one.
        ("sitemap", &["pane structure", "structure.new"]),
        // The Terminal strips navigator and the command that places a terminal.
        ("ew-terminal-strip", &["pane strips", "tool.place_terminal"]),
        // The Cables navigator and the command that makes the selected wires a cable.
        ("ew-cable", &["pane cables", "cable.from_wires"]),
        // A QElectroTech project imported, and the same file imported again into its import.
        ("file-import", &["file.import_qet", "file.reimport_qet"]),
    ];
    let mut by_key: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for a in ew_viewmodel::actions::ACTIONS {
        if let (Some(_), Some(icon)) = (a.placement, a.icon) {
            by_key.entry(icon).or_default().insert(a.id.to_owned());
        }
    }
    for (_, _, icon, action) in ew_viewmodel::rail::RIGHT {
        by_key.entry(icon).or_default().insert(action.to_owned());
    }
    for (word, icon, _) in ew_viewmodel::rail::LEFT {
        by_key.entry(icon).or_default().insert(format!("pane {word}"));
    }
    let mut twice: Vec<(&str, Vec<String>)> = Vec::new();
    for (k, concepts) in by_key {
        let allowed: BTreeSet<String> = SHARED
            .iter()
            .filter(|(s, _)| *s == k)
            .flat_map(|(_, c)| c.iter().map(|c| (*c).to_owned()))
            .collect();
        let rest: Vec<String> = concepts.difference(&allowed).cloned().collect();
        if concepts.len() > 1 && !rest.is_empty() {
            twice.push((k, concepts.into_iter().collect()));
        }
    }
    assert_eq!(twice, Vec::<(&str, Vec<String>)>::new());
}
