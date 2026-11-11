// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 5, the canvas overlays (spec §9, acceptance checks §12.5): the CPU snapshots of folio 101 with a
//! selection and four highlights, a measurement and a Place ghost, blessed under `tests/expected/canvas/` and equal
//! across two runs (OV5); the selection's 2-pixel outline, white casing and four 8-pixel handles probed in the
//! snapshot (OV6); the pick stack's hover outline and the walk (OV7); in the window, the highlight name chip's model
//! values (OV6), the link preview's accessible node (OV4) and the chrome obstacles the chips keep off (OV2,
//! FT-508).
//!
//! `EW_BLESS=1` writes the blessed images. They are compared pixel by pixel with a channel tolerance of 4 and at most
//! 0.05 % differing pixels (the CPU raster's text edges may differ by a unit between platforms).

use crate::support;

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_render_cpu::{RgbaImage, SheetSize};
use ew_render2d::overlay::{self, OverlayInput};
use ew_render2d::view::{PxRect, View};
use ew_ui::app::App;
use ew_ui::canvas_chips::ChipKind;
use ew_ui::ew_viewmodel::canvas::HOVER_SLOP_PT;
use ew_ui::ew_viewmodel::canvas::chip::CHIP_OFFSET;
use ew_ui::ew_viewmodel::ew_api::output::ew_dl::ew_units::{Nm, Rect as SheetRect};
use ew_ui::ew_viewmodel::highlight::SLOT_COLOURS;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Mode, ShellModel};
use ew_ui::ew_viewmodel::tools::pick_stack;
use serde_json::{Value, json};
use support::{Tmp, dispatch, done, on_golden, settle};

const ACCENT: [u8; 3] = [0x0B, 0x63, 0xCE];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// `canvas.open_cp` (FX38a): slate-400, a neutral ink, not the accent.
const OPEN_CP: [u8; 3] = [0x86, 0x8E, 0x99];

fn at(p: (i64, i64)) -> Value {
    json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6})
}

/// A copy of Golden A open on folio 101 in a headless shell, the pane 800 × 560 points at ppp 1.
fn headless_101(tmp: &Tmp) -> ShellModel {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    done(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 800.0, "height": 560.0, "ppp": 1.0}),
    );
    done(&mut shell, "view.fit_page", json!({}));
    shell
}

fn row_key(shell: &ShellModel, kind: NavKind, role: NavRole, label: &str) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// The four potentials of folio 101 highlighted from the Potentials navigator (slots 0 to 3).
const HIGHLIGHTED: [&str; 4] = ["L", "N", "PE", "L-55"];

fn highlight_four(shell: &mut ShellModel) {
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    for name in HIGHLIGHTED {
        let key = row_key(shell, NavKind::Potentials, NavRole::Potential, name);
        done(
            shell,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
    }
}

fn snapshot(shell: &ShellModel) -> RgbaImage {
    ew_ui::canvas::snapshot_image(shell, 0).expect("a snapshot").0
}

fn rgb(img: &RgbaImage, x: i64, y: i64) -> [u8; 3] {
    assert!(x >= 0 && y >= 0 && x < i64::from(img.width) && y < i64::from(img.height));
    let i = (y as usize * img.width as usize + x as usize) * 4;
    [img.pixels[i], img.pixels[i + 1], img.pixels[i + 2]]
}

/// The overlay the snapshot composites for the shown canvas with `hover` and `selected`.
fn overlay_of(shell: &ShellModel, hover: Option<SheetRect>, selected: &[SheetRect]) -> overlay::Overlay {
    let c = shell.canvas().unwrap();
    let view = View {
        centre: c.centre(),
        zoom: c.zoom(),
        pane: c.native_pane(),
        ppp: c.native_ppp(),
    };
    let page = c.page();
    overlay::build(&OverlayInput {
        transform: view.settled_transform(),
        pane_px: view.pane_px_whole(),
        ppp: view.ppp,
        sheet: SheetSize {
            width: Nm(page.sheet.0),
            height: Nm(page.sheet.1),
        },
        grid: None,
        hover,
        selected,
        cursor: None,
        tool: &[],
        open_cps: overlay::OpenCps::default(),
    })
}

// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

fn expected(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/canvas")).join(name)
}

fn decode(bytes: &[u8]) -> RgbaImage {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().expect("a PNG");
    let mut buf = vec![0u8; reader.output_buffer_size().expect("a size")];
    let info = reader.next_frame(&mut buf).expect("a frame");
    buf.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        other => panic!("{other:?}"),
    };
    RgbaImage {
        width: info.width,
        height: info.height,
        pixels,
    }
}

/// OV5: compares `image` with the blessed `name` (or blesses it).
fn gate(name: &str, image: &RgbaImage) {
    let path = expected(name);
    if bless() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, ew_render_cpu::encode_png(image, Some(96))).unwrap();
        return;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{} is blessed (EW_BLESS=1)", path.display()));
    let blessed = decode(&bytes);
    assert_eq!(
        (blessed.width, blessed.height),
        (image.width, image.height),
        "{name}: size"
    );
    let differs: Vec<bool> = blessed
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(image.pixels.as_chunks::<4>().0)
        .map(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 4))
        .collect();
    let differing = differs.iter().filter(|d| **d).count();
    let allowed = (image.width as usize * image.height as usize) / 2000;
    assert!(
        differing <= allowed,
        "{name}: {differing} pixels differ (at most {allowed})"
    );
    if let Some((tx, ty, n)) = crowded_tile(&differs, image.width as usize) {
        panic!("{name}: {n} pixels differ in the tile at ({tx}, {ty}) (at most {TILE_PIXELS})");
    }
}

/// The differing pixels a 32 × 32 tile of a snapshot may hold.
const TILE_PIXELS: usize = 16;

/// The first 32 × 32 tile (x, y, count) of an image `w` pixels wide that holds more than [`TILE_PIXELS`]
/// differing pixels: the differences may be scattered edge pixels, never a drawn thing, so a short tag that appears
/// or goes (59 pixels of "1X1") fails although the whole image allows more. The tiles overlap by half (one
/// every 16 pixels), so a tag up to 16 pixels across lies whole in one of them wherever it stands.
fn crowded_tile(differs: &[bool], w: usize) -> Option<(usize, usize, usize)> {
    let h = differs.len() / w;
    (0..h).step_by(16).find_map(|ty| {
        (0..w).step_by(16).find_map(|tx| {
            let n = (ty..(ty + 32).min(h))
                .flat_map(|y| (tx..(tx + 32).min(w)).map(move |x| y * w + x))
                .filter(|i| differs[*i])
                .count();
            (n > TILE_PIXELS).then_some((tx, ty, n))
        })
    })
}

/// An 8 × 8 block (a short tag) is a crowded tile, 16 scattered pixels are not; FT-1350: so is one that
/// straddles the corner of four tiles, 4 × 4 pixels in each.
#[test]
fn a_short_tag_is_a_crowded_tile() {
    let (w, h) = (640, 700);
    let block = |x0: usize, y0: usize| {
        let mut tag = vec![false; w * h];
        for y in y0..y0 + 8 {
            for x in x0..x0 + 8 {
                tag[y * w + x] = true;
            }
        }
        tag
    };
    assert_eq!(crowded_tile(&block(200, 100), w), Some((176, 80, 64)));
    assert_eq!(crowded_tile(&block(220, 92), w), Some((208, 64, 32)));
    let mut scattered = vec![false; w * h];
    for i in 0..16 {
        scattered[i * 40 * w + i * 37] = true;
    }
    assert_eq!(crowded_tile(&scattered, w), None);
}

/// OV5 and OV6: folio 101 with a selection (the snapshot probed: the 2-pixel outline in `canvas.selection`, the white
/// casing on both sides, four 8-pixel handles with an accent rim and a white face), then with four highlights, a
/// measurement and a Place ghost; each snapshot equal across two runs and to its blessed image.
#[test]
fn the_overlay_snapshots_of_folio_101() {
    let tmp = Tmp::new();
    let mut shell = headless_101(&tmp);
    // The first representation of the page tables whose selection is one rectangle.
    let reps = shell.canvas().unwrap().page().tables.reps.clone();
    let mut selected = Vec::new();
    for rep in &reps {
        done(&mut shell, "select.set", json!({"ids": [rep.id.to_string()]}));
        selected = shell.selected_rects(0);
        if selected.len() == 1 {
            break;
        }
    }
    assert_eq!(selected.len(), 1, "a representation selected as one rectangle");
    let image = snapshot(&shell);
    assert_eq!(image.pixels, snapshot(&shell).pixels, "stable");
    let o = overlay_of(&shell, None, &selected);
    assert_eq!(o.handles.len(), 4);
    for h in &o.handles {
        assert_eq!((h.x1 - h.x0, h.y1 - h.y0), (8, 8), "an 8-pixel handle");
        assert_eq!(rgb(&image, h.x0, h.y0), ACCENT, "the handle's rim");
        assert_eq!(rgb(&image, h.x0 + 3, h.y0 + 3), WHITE, "the handle's face");
    }
    // The top band's middle: 2 pixels of accent with white above and below.
    let top = o
        .selected
        .iter()
        .min_by_key(|r| (r.y0, r.x0))
        .copied()
        .unwrap_or(PxRect::new(0, 0, 0, 0));
    assert_eq!(top.y1 - top.y0, 2, "a 2-pixel outline");
    let mid = (top.x0 + top.x1) / 2;
    assert_eq!(rgb(&image, mid, top.y0), ACCENT);
    assert_eq!(rgb(&image, mid, top.y0 + 1), ACCENT);
    assert_eq!(rgb(&image, mid, top.y0 - 1), WHITE, "the outer casing");
    assert_eq!(rgb(&image, mid, top.y0 + 2), WHITE, "the inner casing");
    gate("101-selection.png", &image);

    // Four highlights over the selection.
    highlight_four(&mut shell);
    assert_eq!(
        shell.highlight_overlay(0).len(),
        4,
        "the four potentials are drawn on 101"
    );
    let image = snapshot(&shell);
    assert_eq!(image.pixels, snapshot(&shell).pixels, "stable");
    let hues = |img: &RgbaImage| {
        SLOT_COLOURS
            .iter()
            .filter(|(r, g, b)| {
                img.pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[..3] == [*r, *g, *b])
            })
            .count()
    };
    assert_eq!(hues(&image), 4, "every slot's hue shows");
    gate("101-selection-highlights.png", &image);

    shell.shutdown();

    // A measurement between two connection points (as T7 measures): the line in `canvas.measure`.
    let mut shell = headless_101(&tmp);
    for (action, params) in [
        ("tool.measure", json!({})),
        ("cp.next", json!({})),
        ("canvas.activate", json!({})),
        ("cp.next", json!({})),
        ("cp.next", json!({})),
        ("canvas.activate", json!({})),
        ("cursor.set", json!({})),
    ] {
        done(&mut shell, action, params);
    }
    let image = snapshot(&shell);
    assert_eq!(image.pixels, snapshot(&shell).pixels, "stable");
    let count = |img: &RgbaImage, c: [u8; 3]| {
        img.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[..3] == c)
            .count()
    };
    let measure = [0x1A, 0x1D, 0x21];
    assert!(count(&image, measure) > 20, "the measure line");
    gate("101-measure.png", &image);
    shell.shutdown();

    // A Place ghost: its outline dashed in `canvas.ghost`, the accent at 60 % over the sheet.
    let mut shell = headless_101(&tmp);
    done(&mut shell, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    let (w, h) = shell.canvas().unwrap().sheet();
    done(&mut shell, "canvas.pointer", at((w / 3, h * 2 / 3)));
    assert!(shell.tool_ghost().is_some(), "a ghost");
    let image = snapshot(&shell);
    assert_eq!(image.pixels, snapshot(&shell).pixels, "stable");
    let blend = |src: u8| ((u32::from(src) * 153 + 255 * 102 + 127) / 255) as u8;
    let ghost = [blend(ACCENT[0]), blend(ACCENT[1]), blend(ACCENT[2])];
    assert!(count(&image, ghost) > 20, "the ghost's outline");
    gate("101-ghost.png", &image);
    shell.shutdown();
}

/// OV7 (FT-500 kept, doc 16a §6.25): on folio 101, at a point of a wire's route where the pick stack holds more than
/// one entity, the hover outline is drawn dashed in `canvas.hover` around the stack's first entity; a click selects
/// it, a repeated click the next one, drawn in `canvas.selection`.
#[test]
fn the_pick_stacks_hover_outline_and_walk() {
    let tmp = Tmp::new();
    let mut shell = headless_101(&tmp);
    let c = shell.canvas().unwrap();
    let page = c.page();
    let reach = HOVER_SLOP_PT / c.points_per_nm();
    // The middle of the first wire segment whose pick stack holds two entities or more, off every link.
    let p = page
        .tables
        .connections
        .iter()
        .flat_map(|t| {
            t.polyline
                .windows(2)
                .map(|w| {
                    page.snapper
                        .page_to_sheet(((w[0].0 + w[1].0) / 2, (w[0].1 + w[1].1) / 2))
                })
                .collect::<Vec<_>>()
        })
        .find(|p| c.link_at(*p).is_none() && pick_stack(page, &page.snapper, *p, reach).len() >= 2)
        .expect("a point where entities overlap");
    let stack = pick_stack(page, &page.snapper, p, reach);
    let rect_of = |e| {
        page.entities
            .iter()
            .find(|(id, _)| *id == e)
            .map(|(_, r)| *r)
            .expect("the entity's rectangle")
    };
    let (first, second) = (rect_of(stack[0]), rect_of(stack[1]));
    done(&mut shell, "canvas.pointer", at(p));
    let c = shell.canvas().unwrap();
    assert_eq!(
        ew_ui::canvas::hover_rect(&shell, 0, c),
        Some(first),
        "the hover is the stack's first entity"
    );
    let image = snapshot(&shell);
    let o = overlay_of(&shell, Some(first), &[]);
    assert!(!o.hover.is_empty());
    // Dashed: every dash in the accent; the dashes of the top edge 8 pixels apart (4 on, 4 off).
    for d in &o.hover {
        assert_eq!(rgb(&image, d.x0, d.y0), ACCENT, "a hover dash at {d:?}");
    }
    let top = o.hover.iter().map(|d| d.y0).min().unwrap();
    let mut dashes: Vec<PxRect> = o
        .hover
        .iter()
        .filter(|d| d.y0 == top && d.x1 - d.x0 > 1)
        .copied()
        .collect();
    dashes.sort_by_key(|d| d.x0);
    assert!(dashes.len() >= 2);
    assert_eq!(dashes[1].x0 - dashes[0].x0, 8, "4 on, 4 off");
    // A click selects the first, a repeated click the next (the walk as built).
    done(&mut shell, "canvas.press", at(p));
    done(&mut shell, "canvas.release", at(p));
    assert_eq!(shell.selection().items, vec![stack[0]]);
    done(&mut shell, "canvas.press", at(p));
    done(&mut shell, "canvas.release", at(p));
    assert_eq!(shell.selection().items, vec![stack[1]]);
    assert!(
        shell.selected_rects(0).contains(&second),
        "{:?}",
        shell.selected_rects(0)
    );
    let image = snapshot(&shell);
    let o = overlay_of(&shell, None, &[second]);
    let band = o.selected.iter().max_by_key(|r| r.x1 - r.x0).copied().unwrap();
    assert_eq!(
        rgb(&image, (band.x0 + band.x1) / 2, band.y0),
        ACCENT,
        "the walked selection"
    );
    shell.shutdown();
}

// -----------------------------------------------------------------------------------------------------------------
// In the window

/// OV6 (model values): the highlight name chips of folio 101 are filled with their slot's hue, with a black label in
/// `type.caption` SemiBold, radius 4 and 20 points high, and they keep off each other.
#[test]
fn the_highlight_name_chips_take_their_hue_and_a_black_label() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "navigator.show", json!({"kind": "potentials"}));
    settle(&mut h);
    for name in HIGHLIGHTED {
        let key = row_key(h.state().shell(), NavKind::Potentials, NavRole::Potential, name);
        dispatch(
            &mut h,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
    }
    settle(&mut h);
    let chips: Vec<_> = h
        .state_mut()
        .canvas_host()
        .canvas_chips(0)
        .iter()
        .filter(|c| matches!(c.kind, ChipKind::Highlight(_)))
        .cloned()
        .collect();
    assert_eq!(chips.len(), 4, "{chips:?}");
    let mut names: Vec<&str> = chips.iter().map(|c| c.text.as_str()).collect();
    names.sort_unstable();
    let mut want = HIGHLIGHTED.to_vec();
    want.sort_unstable();
    assert_eq!(names, want);
    for (i, chip) in chips.iter().enumerate() {
        let ChipKind::Highlight(slot) = chip.kind else {
            unreachable!()
        };
        let (r, g, b) = SLOT_COLOURS[usize::from(slot)];
        assert_eq!(
            (chip.fill.r, chip.fill.g, chip.fill.b, chip.fill.a),
            (r, g, b, 255)
        );
        assert_eq!(
            (chip.text_colour.r, chip.text_colour.g, chip.text_colour.b),
            (0, 0, 0),
            "a black label"
        );
        assert_eq!(chip.radius, 4.0);
        assert_eq!(chip.rect.height(), 20.0);
        assert!(!chip.faded);
        for other in &chips[i + 1..] {
            assert!(!chip.rect.intersects(other.rect), "{chip:?} and {other:?} apart");
        }
    }
    h.state_mut().shell_mut().shutdown();
}

/// FT-1288 (W49.2 item 6): with four potentials of folio 103 highlighted, at 1280 × 720, at the 1280 × 800 floor and
/// at 1920 × 1080, with and without the find pane in the bottom dock, the name chips keep off each other, stand
/// inside the page view and are never faded, and the page view ends above the status bar.
#[test]
fn the_highlight_chips_stay_clear_in_a_short_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    dispatch(&mut h, "navigator.show", json!({"kind": "potentials"}));
    settle(&mut h);
    for name in ["24VDC-1", "0V", "PE", "24VDC-2"] {
        let key = row_key(h.state().shell(), NavKind::Potentials, NavRole::Potential, name);
        dispatch(
            &mut h,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
    }
    for find in [false, true] {
        if find {
            dispatch(&mut h, "find.open", json!({}));
        }
        for size in [
            egui::vec2(1280.0, 720.0),
            egui::vec2(1280.0, 800.0),
            egui::vec2(1920.0, 1080.0),
        ] {
            h.set_size(size);
            settle(&mut h);
            settle(&mut h);
            let status_top =
                h.ctx.content_rect().shrink(8.0).bottom() - ew_ui::theme::current(&h.ctx).size_status;
            let pane = h.state().command_surface_rects().2.expect("the page view");
            assert!(
                pane.bottom() <= status_top + 0.5,
                "{size:?} find {find}: {pane:?} above {status_top}"
            );
            let chips: Vec<_> = h
                .state_mut()
                .canvas_host()
                .canvas_chips(0)
                .iter()
                .filter(|c| matches!(c.kind, ChipKind::Highlight(_)))
                .cloned()
                .collect();
            assert_eq!(chips.len(), 4, "{size:?} find {find}: {chips:?}");
            for (i, chip) in chips.iter().enumerate() {
                assert!(!chip.faded, "{size:?} find {find}: {chip:?}");
                assert!(
                    pane.contains_rect(chip.rect),
                    "{size:?} find {find}: {chip:?} in {pane:?}"
                );
                for other in &chips[i + 1..] {
                    assert!(
                        !chip.rect.intersects(other.rect),
                        "{size:?} find {find}: {chip:?} and {other:?} apart"
                    );
                }
            }
        }
    }
    h.state_mut().shell_mut().shutdown();
}

/// OV4, FT-390: the link preview popup has a named, non-focusable image node with the target's folio and cell.
#[test]
fn the_link_preview_has_a_named_region() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let link = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .find(|l| {
            l.target
                .as_ref()
                .is_some_and(|t| t.folio != "101" && t.cell.is_some())
        })
        .cloned()
        .expect("a link to a cell of another page");
    let target = link.target.clone().unwrap();
    let p = ew_ui::ew_viewmodel::canvas::rect_centre(link.rect);
    // The window's pointer rests over the link (its screen point from the canvas's view) until the preview shows.
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    let pos = egui::Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    );
    h.event(egui::Event::PointerMoved(pos));
    for _ in 0..200 {
        if h.state().shell().link_preview().is_some() {
            break;
        }
        h.run_ok();
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(
        h.state().shell().link_preview().is_some(),
        "the rest shows the preview"
    );
    h.run_ok();
    let name = format!(
        "Link preview: page {}, cell {}",
        target.folio,
        target.cell.as_deref().unwrap()
    );
    let node = h.get_by_label(&name);
    assert_eq!(node.accesskit_node().role(), egui::accesskit::Role::Image);
    assert!(
        !node
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus),
        "not focusable"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1091 (play-test rv-vis-7, t0009): the link preview popup and a hover tooltip never show at once. A
/// connection point that stands on a link (24VDC-1 of folio 103, over 0VDC's link to cell 102-C15; FT-1436 moved
/// folio 101's "102-A1" off N-1) is hovered until the rest opens the popup, and then for as long again: the connection
/// point's tooltip does not show while the popup does.
///
/// No connection point of Golden A stands on a link since WP-S2.19 (the target form's connection point is at the
/// arrow's tip, D16, and the slot texts keep clear of the points), so the case has no page to run on until a page is
/// built for it (FT-1596).
#[test]
#[ignore = "FT-1596: no Golden A page has a connection point on a link since WP-S2.19 (D16)"]
fn ft1091_the_link_preview_and_the_cp_tooltip_never_show_together() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let (cp, link) = {
        let c = h.state().shell().canvas().unwrap();
        let page = c.page();
        let link = page
            .links
            .iter()
            .find(|l| {
                l.target.as_ref().is_some_and(|t| t.folio != "101")
                    && page.cps.iter().any(|cp| {
                        let (r, p) = (l.rect, cp.at_nm);
                        p.0 >= r.min().x.0 && p.0 <= r.max().x.0 && p.1 >= r.min().y.0 && p.1 <= r.max().y.0
                    })
            })
            .cloned()
            .expect("a link with a connection point on it");
        let cp = page
            .cps
            .iter()
            .find(|cp| {
                let (r, p) = (link.rect, cp.at_nm);
                p.0 >= r.min().x.0 && p.0 <= r.max().x.0 && p.1 >= r.min().y.0 && p.1 <= r.max().y.0
            })
            .cloned()
            .expect("the connection point");
        (cp, link)
    };
    assert_eq!(cp.label, "24VDC-1");
    assert_eq!(link.target.as_ref().unwrap().cell.as_deref(), Some("102-C15"));
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    let pos = egui::Pos2::new(
        bounds.x0 as f32 + ((cp.at_nm.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((cp.at_nm.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    );
    h.event(egui::Event::PointerMoved(pos));
    // The model learns the pointer in the first frames; the tooltip's text is its.
    let mut tip = String::new();
    for _ in 0..200 {
        if h.state().shell().link_preview().is_some() {
            break;
        }
        h.run_ok();
        if tip.is_empty() {
            tip = h
                .state()
                .shell()
                .canvas()
                .and_then(|c| c.hovered_cp_tip())
                .unwrap_or_default();
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(
        h.state().shell().link_preview().is_some(),
        "the rest shows the preview"
    );
    assert!(
        tip.starts_with("24VDC-1"),
        "the connection point's tooltip text: {tip:?}"
    );
    // The popup stays while the pointer rests: for 120 more frames its tooltip never shows beside it.
    for frame in 0..120 {
        h.run_ok();
        assert!(
            h.state().shell().link_preview().is_some(),
            "frame {frame}: the popup stays"
        );
        assert!(
            h.query_by_label(&tip).is_none(),
            "frame {frame}: the tooltip {tip:?} waits while the popup shows"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.state_mut().shell_mut().shutdown();
}

/// OV2, OV6, FT-508: the window reports the tool HUD's and the zoom control's frames over the canvas
/// (`chrome_obstacles` holds both). The Place ghost is aimed so that its label's preferred spot (measured in free
/// space mid-sheet) overlaps the top left corner of the HUD and then of the zoom control; with the frames as
/// obstacles the label moves to a free candidate (not faded) and no canvas chip stands on either.
#[test]
fn the_chips_keep_off_the_hud_and_the_zoom_control() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    // W49.3b-F7: the label ("Variant A · new F") is wider than the variant letter alone was; a wider window keeps
    // room for it above left of the HUD, where the test expects a free spot.
    h.set_size(egui::Vec2::new(1400.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    settle(&mut h);
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    // Points are mapped with the transform the last frame drew (and placed the chips) with, the settled one
    // once the view rests, not the model's exact zoom, which differs by a few points away from the pane's centre
    // (as the tools tests since fc1fbd97); whether the view has rested then depends on how many frames ran.
    let drawn = |h: &mut Harness<'_, App>| {
        let host = h.state_mut().canvas_host();
        let a = host.drawn_screen_of(0, (0, 0)).expect("a drawn canvas");
        let b = host.drawn_screen_of(0, (w, hh)).expect("a drawn canvas");
        (
            a,
            f64::from(b.x - a.x) / w as f64,
            f64::from(b.y - a.y) / hh as f64,
        )
    };
    let to_screen = |map: (egui::Pos2, f64, f64), p: (i64, i64)| {
        map.0 + egui::vec2((p.0 as f64 * map.1) as f32, (p.1 as f64 * map.2) as f32)
    };
    let to_sheet = |map: (egui::Pos2, f64, f64), s: egui::Pos2| {
        (
            (f64::from(s.x - map.0.x) / map.1) as i64,
            (f64::from(s.y - map.0.y) / map.2) as i64,
        )
    };
    let label = |h: &mut Harness<'_, App>| {
        h.state_mut()
            .canvas_host()
            .canvas_chips(0)
            .iter()
            .find(|c| c.kind == ChipKind::Label)
            .map(|c| c.rect)
            .expect("the ghost's label")
    };
    // The label's preferred spot relative to the ghost, in free space.
    let free = (w / 3, hh / 3);
    dispatch(&mut h, "canvas.pointer", at(free));
    settle(&mut h);
    let free_label = label(&mut h);
    let mut map = drawn(&mut h);
    let offset = free_label.center() - to_screen(map, free);
    let size = free_label.size();
    let (_, hud, _) = h.state().command_surface_rects();
    let hud = hud.expect("the HUD over the page view");
    let chrome = h.state_mut().canvas_host().chrome_obstacles(0).to_vec();
    assert_eq!(chrome.len(), 2, "the HUD and the zoom control: {chrome:?}");
    assert_eq!(chrome[0], hud);
    let zoom = chrome[1];
    assert!(!zoom.intersects(hud), "two frames: {zoom:?} {hud:?}");
    for (name, frame) in [("HUD", hud), ("zoom control", zoom)] {
        // The label's anchor by the frame's top left corner (6 points in, so above left still fits on the canvas):
        // the preferred spot (above right) and the next (below right) overlap the frame, above left is free.
        let anchor = egui::pos2(frame.left() - CHIP_OFFSET + 6.0, frame.top() + 12.0);
        let spot = anchor + egui::vec2(CHIP_OFFSET + size.x / 2.0, -CHIP_OFFSET - size.y / 2.0);
        let target = to_sheet(map, spot - offset);
        dispatch(&mut h, "canvas.pointer", at(target));
        settle(&mut h);
        settle(&mut h);
        // Without the obstacles the label would stand on the frame.
        map = drawn(&mut h);
        let preferred = egui::Rect::from_center_size(to_screen(map, target) + offset, label(&mut h).size());
        assert!(
            preferred.intersects(frame),
            "{preferred:?} would stand on the {name} {frame:?}"
        );
        let chips = h.state_mut().canvas_host().canvas_chips(0).to_vec();
        for c in &chips {
            assert!(
                !c.faded,
                "{:?} placed on a free candidate (aimed at the {name})",
                c.kind
            );
            assert!(
                !c.rect.intersects(hud) && !c.rect.intersects(zoom),
                "{:?} {:?} off the HUD {hud:?} and the zoom control {zoom:?} (aimed at the {name})",
                c.kind,
                c.rect
            );
        }
    }
    h.state_mut().shell_mut().shutdown();
}

/// The free connection points of Golden A's first multi-line page that has some (spare terminals are unwired by
/// design) marked open in the canvas snapshot at 100 % (a hollow square in `canvas.open_cp`, a neutral grey, FX38a:
/// its corners grey, no accent on the plain page), none at
/// 25 % (a grid step under 4 points), and only in the session overlay: the page's display list, which print and PDF
/// draw, does not change.
#[test]
fn free_cps_are_marked_open_on_screen_only() {
    let tmp = Tmp::new();
    let mut shell = headless_101(&tmp);
    let folios: Vec<String> = shell
        .page_rows()
        .iter()
        .filter(|r| r.kind == "multi_line")
        .map(|r| r.folio.clone())
        .collect();
    let folio = folios
        .into_iter()
        .find(|f| {
            done(&mut shell, "page.goto", json!({ "folio": f }));
            !shell.open_cps(0).is_empty()
        })
        .expect("a page with free connection points");
    let open = shell.open_cps(0);
    assert!(!open.is_empty(), "{folio} has free connection points");
    let items = shell.canvas().unwrap().page().list.items.len();
    // At 25 % the step is under 4 points: no mark.
    done(&mut shell, "view.zoom_to", json!({"percent": 25.0}));
    let far = snapshot(&shell);
    let far_marks = (0..far.width as usize * far.height as usize)
        .filter(|i| far.pixels[i * 4..i * 4 + 3] == OPEN_CP)
        .count();
    assert_eq!(far_marks, 0, "no mark at 25 %");
    done(&mut shell, "view.zoom_actual", json!({}));
    let c = shell.canvas().unwrap();
    let view = View {
        centre: c.centre(),
        zoom: c.zoom(),
        pane: c.native_pane(),
        ppp: c.native_ppp(),
    };
    let t = view.settled_transform();
    let image = snapshot(&shell);
    let accent = (0..image.width as usize * image.height as usize)
        .filter(|i| image.pixels[i * 4..i * 4 + 3] == ACCENT)
        .count();
    assert_eq!(
        accent, 0,
        "no accent on the plain page: the open marks are not handles"
    );
    // The CPs on screen, away from the pane's edges.
    let (w, h) = (i64::from(image.width), i64::from(image.height));
    let shown: Vec<(i64, i64)> = open
        .iter()
        .map(|p| {
            let (x, y) = t.to_screen((p.0 as f64, p.1 as f64));
            (x.floor() as i64, y.floor() as i64)
        })
        .filter(|(x, y)| *x >= 4 && *y >= 4 && *x < w - 4 && *y < h - 4)
        .collect();
    assert!(!shown.is_empty(), "a free CP on screen at 100 %");
    for (x, y) in &shown {
        for (dx, dy) in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
            assert_eq!(
                rgb(&image, x + dx, y + dy),
                OPEN_CP,
                "the mark's corner at {x},{y}"
            );
        }
    }
    assert_eq!(shell.canvas().unwrap().page().list.items.len(), items);
}
