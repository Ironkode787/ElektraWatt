// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chrome snapshots of WP-S2.17 (§11 item 3, DL6, DK7): the main window on Golden A folio 101 at 1280 × 800 in
//! light and dark at both densities, and the component gallery in the four themes, rendered offscreen (`offscreen`)
//! and written as PNG files under `target/ew-ui-snapshots/` (or `EW_SNAPSHOT_DIR`) for the reviewer, who compares
//! them with the mockup side by side.
//!
//! The pixel gate (D7 as amended: no `egui_kittest` render feature): the main window in light and dark at both
//! densities and the gallery in the four themes at ppp 1 are compared with the blessed PNG files under
//! `tests/expected/snapshots/`. A pixel differs when a channel differs by more than
//! [`offscreen::CHANNEL_TOLERANCE`]; an image fails with more than [`offscreen::MAX_DIFFERING_PIXELS`] differing
//! pixels, and a diff image (differences in red) is written next to the output. `EW_BLESS=1` writes the blessed
//! files. Behind the `snapshots` feature and on Windows only (DX12 WARP: one platform keeps the pixels stable); no
//! adapter there is a failure.
//!
//! One image per screen (test audit, 2026-10-29): each cropped screen is gated in light Comfortable, plus dark
//! Compact for the DK12 panes. The theme tokens are gated by `theme.rs` (`dl2_the_dump_equals_the_blessed_file`,
//! `dl2_density_changes_sizes_only`, `dl3_apply_maps_the_tokens_onto_egui`) and the whole window in light and dark at
//! both densities by `main-*`, so a dark or Compact copy of every crop guards nothing more. Every image this file
//! writes is gated: nothing is rendered for review only.

#![cfg(all(feature = "snapshots", windows))]

mod gallery;
mod offscreen;
mod support;

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::{App, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;
use support::{Scripted, Tmp};

/// Where the images go.
// Program boundary of the test binary: the reviewer's output directory; no image depends on it.
#[allow(clippy::disallowed_methods)]
fn out_dir() -> PathBuf {
    std::env::var_os("EW_SNAPSHOT_DIR").map_or_else(
        || {
            PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../target/ew-ui-snapshots"
            ))
        },
        PathBuf::from,
    )
}

/// Whether this run writes the blessed images.
// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

fn device() -> ew_render2d::gpu::HeadlessDevice {
    offscreen::device().expect("no adapter: on Windows DX12 WARP at least must be found")
}

/// Writes `image` for review as `name` and, when `gated`, compares it with the blessed file (or blesses it). Returns
/// the failure, if any.
fn record(name: &str, image: &ew_render_cpu::RgbaImage, gated: bool) -> Option<String> {
    let out = out_dir();
    offscreen::write_png(image, &out.join(format!("{name}.png")));
    if !gated {
        return None;
    }
    let blessed = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/snapshots"))
        .join(format!("{name}.png"));
    if bless() {
        offscreen::write_png(image, &blessed);
        return None;
    }
    let Some(old) = offscreen::read_png(&blessed) else {
        return Some(format!("{name}: no blessed image (run with EW_BLESS=1)"));
    };
    match offscreen::compare(&old, image) {
        None => Some(format!(
            "{name}: {}x{} against the blessed {}x{}",
            image.width, image.height, old.width, old.height
        )),
        Some((count, diff)) if count > offscreen::MAX_DIFFERING_PIXELS => {
            let path = out.join(format!("{name}.diff.png"));
            offscreen::write_png(&diff, &path);
            Some(format!(
                "{name}: {count} pixels differ (at most {} may); see {}",
                offscreen::MAX_DIFFERING_PIXELS,
                path.display()
            ))
        }
        Some(_) => None,
    }
}

/// The main window on a copy of Golden A in `theme` and `density`, with the temporary directory that holds the copy:
/// the caller drops it after the harness, so no copy is left in the temporary directory.
fn main_window(theme: &str, density: &str) -> (Harness<'static, App>, offscreen::Recorder, Tmp) {
    main_window_at(theme, density, 1.0)
}

/// [`main_window`] at `ppp` physical pixels per point (the system's display scale; the window stays 1280 × 800
/// points).
fn main_window_at(theme: &str, density: &str, ppp: f32) -> (Harness<'static, App>, offscreen::Recorder, Tmp) {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let (h, recorder) = window_on(tmp.config(), dir, theme, density, ppp, true);
    (h, recorder, tmp)
}

/// The main window with `config` on the project `dir` (a copy the test may have prepared) in `theme` and `density` at
/// `ppp`, with its page on show; on folio 101 when `goto_101` (the recovery dialog refuses the action).
fn window_on(
    config: ShellConfig,
    dir: PathBuf,
    theme: &str,
    density: &str,
    ppp: f32,
    goto_101: bool,
) -> (Harness<'static, App>, offscreen::Recorder) {
    let mut shell = ShellModel::new(Mode::Gui, config);
    assert_eq!(
        shell.dispatch("settings.set", &json!({"key": "theme", "value": theme})),
        Outcome::Done
    );
    // A base without the density setting refuses the key; the image is then Comfortable.
    let _ = shell.dispatch("settings.set", &json!({"key": "density", "value": density}));
    let recorder = offscreen::Recorder::default();
    let app = App::new(shell, Box::new(Scripted::default()), Some(dir));
    let mut h = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
        .with_pixels_per_point(ppp)
        .with_context_setup(setup)
        .renderer(recorder.clone())
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    h.run_ok();
    support::wait_for_page(&mut h);
    if goto_101 {
        assert_eq!(
            h.state_mut()
                .shell_mut()
                .dispatch("page.goto", &json!({"folio": "101"})),
            Outcome::Done
        );
        h.run_ok();
        support::wait_for_page(&mut h);
    }
    h.run_ok();
    (h, recorder)
}

/// The main window in light and dark at both densities (the high-contrast themes are gated by the gallery).
#[test]
fn the_main_window_in_light_and_dark_at_both_densities() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark"] {
        for density in ["comfortable", "compact"] {
            let (h, recorder, tmp) = main_window(theme, density);
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            assert_eq!((image.width, image.height), (1280, 800));
            failures.extend(record(&format!("main-{theme}-{density}"), &image, true));
            // The project closes before its directory is removed.
            drop(h);
            drop(tmp);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// DK7's 300 % variant: the main window, light and Comfortable, at 1280 × 800 points on a 300 % display (3840 × 2400
/// pixels). The setting `ui_scale` stops at 200 % (`UI_SCALES`), so 300 % is the system's scale (egui's native pixels
/// per point), which is how a 300 % display reaches the window. Gated since FT-537 (the orchestrator's ruling) and
/// re-blessed with the main-window images; the test keeps its name from before the ruling.
#[test]
fn the_main_window_at_300_percent_for_review() {
    let dev = device();
    let (h, recorder, tmp) = main_window_at("light", "comfortable", 3.0);
    let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
    assert_eq!((image.width, image.height), (3840, 2400));
    assert_eq!(record("main-light-comfortable-300", &image, true), None);
    drop(h);
    drop(tmp);
}

/// The component gallery in the four themes at ppp 1 (the only gated image of the high-contrast themes).
#[test]
fn the_component_gallery_in_every_theme() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        let recorder = offscreen::Recorder::default();
        let mut h = Harness::builder()
            .with_size(Vec2::new(640.0, 560.0))
            .with_pixels_per_point(1.0)
            .with_context_setup(setup)
            .renderer(recorder.clone())
            .build_ui_state(move |ui, _: &mut ()| gallery::show(ui, theme), ());
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        failures.extend(record(&format!("gallery-{theme}-ppp1"), &image, true));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `image`'s pixels inside `rect` (points at ppp 1), clamped to the image.
/// The vertical centre of the ink (pixels that differ from the field's fill) between `x0` and `x1` inside `field`,
/// 3 px in from its top and bottom (the border).
fn ink_centre(image: &ew_render_cpu::RgbaImage, field: egui::Rect, x0: f32, x1: f32) -> f32 {
    let at = |x: u32, y: u32| {
        let i = ((y * image.width + x) * 4) as usize;
        [image.pixels[i], image.pixels[i + 1], image.pixels[i + 2]]
    };
    let fill = at(field.center().x as u32, (field.top() + 4.0) as u32);
    let ink =
        |y: u32| (x0 as u32..x1 as u32).any(|x| at(x, y).iter().zip(fill).any(|(a, b)| a.abs_diff(b) > 60));
    let rows: Vec<u32> = ((field.top() + 3.0) as u32..(field.bottom() - 3.0) as u32)
        .filter(|y| ink(*y))
        .collect();
    let (first, last) = (
        rows.first().copied().unwrap_or(0),
        rows.last().copied().unwrap_or(0),
    );
    (first + last) as f32 / 2.0
}

fn crop(image: &ew_render_cpu::RgbaImage, rect: egui::Rect) -> ew_render_cpu::RgbaImage {
    let x0 = (rect.left().max(0.0) as u32).min(image.width);
    let y0 = (rect.top().max(0.0) as u32).min(image.height);
    let x1 = (rect.right().ceil() as u32).clamp(x0, image.width);
    let y1 = (rect.bottom().ceil() as u32).clamp(y0, image.height);
    let mut pixels = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for y in y0..y1 {
        let start = ((y * image.width + x0) * 4) as usize;
        pixels.extend_from_slice(&image.pixels[start..start + ((x1 - x0) * 4) as usize]);
    }
    ew_render_cpu::RgbaImage {
        width: x1 - x0,
        height: y1 - y0,
        pixels,
    }
}

/// CB7 (§12.2): the command bar alone, cropped to its frame, for each of the six tabs, light Comfortable (gated; the
/// bar in dark and Compact is in the `main-*` images).
#[test]
fn the_command_bar_on_every_tab() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let (theme, density) = ("light", "comfortable");
        let (mut h, recorder, tmp) = main_window(theme, density);
        for tab in ["project", "page", "draw", "check", "output", "view"] {
            assert_eq!(
                h.state_mut()
                    .shell_mut()
                    .dispatch("view.command_tab", &json!({"tab": tab})),
                Outcome::Done,
                "{tab}"
            );
            h.run_ok();
            h.run_ok();
            let (bar, _, _) = h.state().command_surface_rects();
            let bar = bar.expect("the command bar");
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            failures.extend(record(
                &format!("command-bar-{tab}-{theme}-{density}"),
                &crop(&image, bar),
                true,
            ));
        }
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The luma (Rec. 709 weights, 0 to 255) of the pixel at `x`, `y`.
fn luma(image: &ew_render_cpu::RgbaImage, x: u32, y: u32) -> f32 {
    let i = ((y * image.width + x) * 4) as usize;
    let p = &image.pixels[i..i + 3];
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// WP-S2.17 part 4 (CP6, CP9): the quit prompt over the toast, the app menu's File submenu, the command palette and
/// the Preview tab with the property grid's "Preview open" strip, in light and dark, gated.
#[test]
fn part4_dialogs_toasts_menu_palette_and_preview_in_light_and_dark() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark"] {
        // An edit gives the toast and the dirty project; closing asks to save.
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        // The palette's caret blinks on the wall clock: steady for a stable image.
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("select.set", json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            (
                "props.set",
                json!({"row": "device/function_text", "value": "Spare"}),
            ),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        h.run_ok();
        h.run_ok();
        let before = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let _ = h.state_mut().shell_mut().dispatch("file.close", &json!({}));
        h.run_ok();
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        // FT-557: the scrim dims the whole window, the command bar, the rail, the navigator, Properties and the status
        // bar included, not only the canvas.
        for (x, y, part) in [
            (900, 60, "command bar"),
            (20, 400, "rail"),
            (200, 330, "navigator"),
            (200, 700, "Properties"),
            (500, 780, "status bar"),
            // FT-624: the toast goes below the backdrop and dims with the rest (a plain spot of its fill).
            (1150, 205, "toast"),
        ] {
            let (a, b) = (luma(&before, x, y), luma(&image, x, y));
            // FT-624: visibly, not only measurably: at least 15 % darker (light measured about 21 %, dark about 39 %).
            assert!(
                a - b >= 8.0 && b <= 0.85 * a,
                "{theme}: the scrim dims the {part} ({a} to {b})"
            );
        }
        failures.extend(record(&format!("part4-dialog-toast-{theme}"), &image, true));
        let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!(null));
        // The toast leaves on the wall clock, so it is dismissed before the shots that do not show it.
        let shell = h.state_mut().shell_mut();
        let serials: Vec<u64> = shell.notifications().visible().iter().map(|n| n.serial).collect();
        for serial in serials {
            shell.dismiss_toast(serial);
        }
        h.run_ok();
        // The app menu with its File submenu.
        support::open_menu(&mut h, &["File"]);
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        failures.extend(record(&format!("part4-menu-{theme}"), &image, true));
        h.key_press(egui::Key::Escape);
        h.run_ok();
        h.key_press(egui::Key::Escape);
        h.run_ok();
        // The palette.
        assert_eq!(
            h.state_mut().shell_mut().dispatch("tools.palette", &json!({})),
            Outcome::Done
        );
        h.run_ok();
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        failures.extend(record(&format!("part4-palette-{theme}"), &image, true));
        h.key_press(egui::Key::Escape);
        h.run_ok();
        // A card in the Preview tab and the grid's strip.
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("select.set", json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            ("props.show", json!({"on": true})),
            ("props.set", json!({"row": "device/tag.counter", "value": "9"})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        h.run_ok();
        support::wait_for_page(&mut h);
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        failures.extend(record(&format!("part4-preview-{theme}"), &image, true));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The rectangle of the first node named `name` (its label, or a label node's value).
fn node_rect(h: &Harness<'_, App>, name: &str) -> Option<egui::Rect> {
    use egui_kittest::kittest::NodeT as _;
    h.root().children_recursive().find_map(|n| {
        let a = n.accesskit_node();
        (a.label().as_deref() == Some(name) || a.value().as_deref() == Some(name)).then(|| n.rect())
    })
}

/// The Messages pane's rectangle: from its header to its footer, across its tree.
fn messages_pane(h: &Harness<'_, App>) -> egui::Rect {
    let list = node_rect(h, "Messages navigator").expect("the Messages tree");
    let shell = h.state().shell();
    let top = shell
        .messages_counts()
        .and_then(|c| {
            let scheme = shell.messages_scheme().unwrap_or_default();
            node_rect(
                h,
                &format!(
                    "{scheme} · E {} · W {} · I {} · waived {} · pending {}",
                    c.errors, c.warnings, c.infos, c.waived, c.pending
                ),
            )
        })
        .map_or(list.top(), |r| r.top());
    let bottom = node_rect(h, "Checks support, but do not replace, engineering verification.")
        .map_or(list.bottom(), |r| r.bottom());
    egui::Rect::from_min_max(
        egui::pos2(list.left(), top - 4.0),
        egui::pos2(list.right(), bottom + 4.0),
    )
}

/// DK12 (WP-S2.17 part 3, lane K32): the Messages pane in tree mode (Golden A's error and a pending finding, then the
/// cursor on the pending finding with its detail block) and the History panel (three steps, a save marker, one undone), each cropped to its pane,
/// gated: all three in light Comfortable, the detail block in dark Compact.
#[test]
fn dk12_messages_and_history_in_light_with_one_dark_compact() {
    let dev = device();
    let mut failures = Vec::new();
    for (theme, density) in [("light", "comfortable"), ("dark", "compact")] {
        let light = theme == "light";
        {
            let (mut h, recorder, tmp) = main_window(theme, density);
            assert_eq!(
                h.state_mut().shell_mut().dispatch("messages.show", &json!({})),
                Outcome::Done
            );
            h.run_ok();
            let shell = h.state_mut().shell_mut();
            // An unpaired interruption point on 101: a pending completeness finding there.
            assert_eq!(shell.dispatch("tool.ip", &json!({})), Outcome::Done);
            let (x, y) = {
                let sn = &shell.canvas().unwrap().page().snapper;
                sn.page_to_sheet((sn.page_of_index(60), sn.page_of_index(3)))
            };
            for (action, params) in [
                (
                    "cursor.set",
                    json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6}),
                ),
                ("ip.name", json!({"text": "SNAP"})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            for _ in 0..3 {
                let state: serde_json::Value =
                    serde_json::from_str(&ew_ui::ew_viewmodel::script::state_json(shell)).unwrap();
                if state["canvas"]["ghost"]["direction"] == "target" {
                    break;
                }
                assert_eq!(shell.dispatch("tool.ip_direction", &json!({})), Outcome::Done);
            }
            for (action, params) in [("canvas.activate", json!({})), ("ui.dismiss", json!({}))] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            h.run_ok();
            h.run_ok();
            let shell = h.state_mut().shell_mut();
            assert!(
                shell.messages_counts().is_some_and(|c| c.pending > 0),
                "a pending finding: {:?}",
                shell.messages_counts()
            );
            // The toast of the edit leaves on the wall clock: dismissed for a stable image.
            let serials: Vec<u64> = shell.notifications().visible().iter().map(|n| n.serial).collect();
            for serial in serials {
                shell.dismiss_toast(serial);
            }
            let pending = shell
                .shown_rows(ew_ui::ew_viewmodel::navigators::NavKind::Messages)
                .into_iter()
                .find(|r| r.finding.is_some_and(|f| f.pending))
                .expect("the pending row");
            // First the rows alone (no cursor, no detail block), then the cursor on the pending finding with its
            // detail block (`messages.next` would open another page and end the pending).
            for (name, cursor) in [("messages", false), ("messages-detail", true)] {
                if cursor {
                    assert_eq!(
                        h.state_mut()
                            .shell_mut()
                            .dispatch("navigator.row", &json!({"kind": "messages", "key": pending.key})),
                        Outcome::Done
                    );
                }
                h.run_ok();
                h.run_ok();
                if !light && !cursor {
                    continue;
                }
                let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
                failures.extend(record(
                    &format!("dk12-{name}-{theme}-{density}"),
                    &crop(&image, messages_pane(&h)),
                    true,
                ));
            }
            if !light {
                h.state_mut().shell_mut().shutdown();
                drop(h);
                drop(tmp);
                continue;
            }
            // History: three steps, a save, one undone.
            let shell = h.state_mut().shell_mut();
            for text in ["one", "two", "three"] {
                assert_eq!(
                    shell.dispatch(
                        "command.run",
                        &json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                            "path": "function_text", "value": text}}),
                    ),
                    Outcome::Done
                );
            }
            for (action, params) in [
                ("file.save", json!({})),
                ("edit.undo", json!({})),
                ("history.show", json!({})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            let serials: Vec<u64> = shell.notifications().visible().iter().map(|n| n.serial).collect();
            for serial in serials {
                shell.dismiss_toast(serial);
            }
            h.run_ok();
            h.run_ok();
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            let pane = h
                .get_all_by_role(egui::Role::Group)
                .find(|n| {
                    use egui_kittest::kittest::NodeT as _;
                    n.accesskit_node().label().as_deref() == Some("History")
                })
                .map(|n| n.rect())
                .expect("the History panel");
            failures.extend(record(
                &format!("dk12-history-{theme}-{density}"),
                &crop(&image, pane),
                true,
            ));
            h.state_mut().shell_mut().shutdown();
            drop(h);
            drop(tmp);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A drag with the primary button from `from` by `by`, in eight steps, one frame each.
fn drag(h: &mut Harness<'_, App>, from: egui::Pos2, by: egui::Vec2) {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.input_mut().events.push(egui::Event::PointerMoved(from));
    h.run_ok();
    h.input_mut().events.push(button(from, true));
    h.run_ok();
    for i in 1..=8 {
        let at = from + by * (i as f32 / 8.0);
        h.input_mut().events.push(egui::Event::PointerMoved(at));
        h.run_ok();
    }
    h.input_mut().events.push(button(from + by, false));
    h.run_ok();
    // The pointer leaves the window so that no hover is painted.
    h.input_mut().events.push(egui::Event::PointerGone);
    h.run_ok();
}

/// DK12's grid (FT-532, doc 16a §6.21): Golden A's Terminal strips navigator in table mode, filtered `x1031`, sorted
/// by Terminal, with the derived Potential column moved next to Terminal so that it is on show, two marked rows and
/// the cursor cell in the second (`dk12-grid-*`), then the editing cell opened on the cursor cell
/// (`dk12-grid-edit-*`; the editor opens where the cursor is, so the cursor ring and the editor are two images).
/// Cropped to the pane from its tool line to the grid's bottom, light Comfortable and dark Compact, gated.
#[test]
fn dk12_the_data_grid_in_light_comfortable_and_dark_compact() {
    let dev = device();
    let mut failures = Vec::new();
    for (theme, density) in [("light", "comfortable"), ("dark", "compact")] {
        {
            let (mut h, recorder, tmp) = main_window(theme, density);
            // The editor's caret blinks on the wall clock: steady for a stable image.
            h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
            let shell = h.state_mut().shell_mut();
            for (action, params) in [
                ("props.show", json!({"on": false})),
                ("navigator.show", json!({"kind": "strips"})),
                ("navigator.table", json!({"kind": "strips"})),
                // WP-S2.07's Pos column is hidden: this image is WP-S2.06's grid in its as-built layout.
                ("table.columns", json!({"kind": "strips", "hide": ["pos"]})),
                ("navigator.filter", json!({"kind": "strips", "text": "x1031"})),
                ("navigator.sort", json!({"kind": "strips", "by": "terminal"})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            // The derived Potential column right after Terminal.
            let order: Vec<String> = {
                let nav = shell
                    .nav(ew_ui::ew_viewmodel::navigators::NavKind::Strips)
                    .unwrap();
                let table = nav.table().unwrap();
                let movable: Vec<&ew_ui::ew_viewmodel::ew_api::output::NavColumn> = nav
                    .table_mode()
                    .unwrap()
                    .shown_columns(table)
                    .into_iter()
                    .map(|c| &table.columns[c])
                    .filter(|c| !c.key_column)
                    .collect();
                let derived = movable
                    .iter()
                    .find(|c| c.derived)
                    .map(|c| c.key.clone())
                    .expect("a derived column on show");
                let mut order: Vec<String> = movable
                    .iter()
                    .map(|c| c.key.clone())
                    .filter(|k| *k != derived)
                    .collect();
                order.insert(1, derived);
                order
            };
            for (action, params) in [
                ("table.columns", json!({"kind": "strips", "order": order})),
                ("ui.focus", json!({"area": "navigator", "kind": "strips"})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            support::settle(&mut h);
            // The left column dragged 200 points wider by its splitter, so that the derived column is on show.
            let splitter = h
                .get_all_by_role(egui::Role::Splitter)
                .find(|n| {
                    use egui_kittest::kittest::NodeT as _;
                    n.accesskit_node()
                        .label()
                        .is_some_and(|l| l.ends_with(" and Page view"))
                })
                .map(|n| n.rect().center())
                .expect("the left column's splitter");
            drag(&mut h, splitter, egui::vec2(200.0, 0.0));
            support::settle(&mut h);
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Home);
            support::settle(&mut h);
            h.key_press(egui::Key::ArrowRight);
            support::settle(&mut h);
            h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowDown);
            support::settle(&mut h);
            let shell = h.state_mut().shell_mut();
            assert_eq!(
                shell
                    .nav(ew_ui::ew_viewmodel::navigators::NavKind::Strips)
                    .unwrap()
                    .marked()
                    .len(),
                2,
                "two marked rows"
            );
            let serials: Vec<u64> = shell.notifications().visible().iter().map(|n| n.serial).collect();
            for serial in serials {
                shell.dismiss_toast(serial);
            }
            for (name, edit) in [("grid", false), ("grid-edit", true)] {
                if edit {
                    assert_eq!(
                        h.state_mut()
                            .shell_mut()
                            .dispatch("table.edit", &json!({"kind": "strips"})),
                        Outcome::Done
                    );
                }
                h.run_ok();
                h.run_ok();
                let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
                let grid = node_rect(&h, "Terminal strips table").expect("the grid");
                let top = node_rect(&h, "Table").map_or(grid.top(), |r| r.top());
                let pane = egui::Rect::from_min_max(
                    egui::pos2(grid.left(), top - 4.0),
                    egui::pos2(grid.right(), grid.bottom()),
                );
                if !edit {
                    // Doc 16a §6.5: the filter's text is centred on the field like its leading icon (it sat 3 px high).
                    let field = h
                        .get_all_by_role(egui::Role::TextInput)
                        .map(|n| n.rect())
                        .find(|r| (r.center().y - (top + 12.0)).abs() < 16.0 && r.left() > grid.left())
                        .expect("the filter field");
                    let (icon, text) = (
                        ink_centre(&image, field, field.left() + 6.0, field.left() + 24.0),
                        ink_centre(&image, field, field.left() + 28.0, field.left() + 60.0),
                    );
                    assert!(
                        (icon - text).abs() <= 1.0,
                        "{theme} {density}: filter text centre {text}, icon centre {icon}"
                    );
                }
                failures.extend(record(
                    &format!("dk12-{name}-{theme}-{density}"),
                    &crop(&image, pane),
                    true,
                ));
            }
            h.state_mut().shell_mut().shutdown();
            drop(h);
            drop(tmp);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Dismisses every toast on show: they leave on the wall clock, so no gated image may show one.
fn dismiss_toasts(h: &mut Harness<'_, App>) {
    let shell = h.state_mut().shell_mut();
    let serials: Vec<u64> = shell.notifications().visible().iter().map(|n| n.serial).collect();
    for serial in serials {
        shell.dismiss_toast(serial);
    }
}

/// The smallest rectangle around `rects`, grown by `margin` on every side.
fn around(rects: impl IntoIterator<Item = egui::Rect>, margin: f32) -> egui::Rect {
    rects
        .into_iter()
        .reduce(|a, b| a.union(b))
        .expect("at least one rectangle")
        .expand(margin)
}

/// CP6 (FT-533): every dialog of the shell one by one that Golden A opens with one action, each cropped to the dialog's
/// frame with its shadow, light Comfortable, gated. Not here: the quit prompt (`part4-dialog-toast-*`)
/// and the four dialogs that need a fixture (the recovery dialog, the question of discarding its journal, the
/// check-scheme question and the purge question), gated by [`cp6_the_fixture_dialogs_in_light`] (FT-571).
#[test]
fn cp6_every_dialog_one_by_one_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        // The fields' caret blinks on the wall clock: steady for a stable image.
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        for (name, action) in [
            ("about", "help.about"),
            ("shortcuts", "help.shortcuts"),
            ("notices", "help.notices"),
            ("goto", "nav.goto"),
            ("save-layout", "view.workspace_save"),
            ("new-page", "page.new"),
            ("new-structure", "structure.new"),
            ("new-external-ref", "external_ref.new"),
            ("renumber-pages", "pages.renumber"),
            ("renumber-devices", "devices.renumber"),
        ] {
            assert_eq!(
                h.state_mut().shell_mut().dispatch(action, &json!({})),
                Outcome::Done,
                "{action}"
            );
            failures.extend(shoot_dialog(
                &dev,
                &mut h,
                &recorder,
                &format!("cp6-dialog-{name}-{theme}"),
            ));
            let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!(null));
            h.run_ok();
            assert!(h.state().shell().dialog().is_none(), "{action}: dismissed");
        }
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Records the open dialog as `name`, cropped to the dialog's frame with its shadow, gated; the toasts (they leave on
/// the wall clock) are dismissed first.
fn shoot_dialog(
    dev: &ew_render2d::gpu::HeadlessDevice,
    h: &mut Harness<'static, App>,
    recorder: &offscreen::Recorder,
    name: &str,
) -> Option<String> {
    dismiss_toasts(h);
    // A few frames: the modal's area sizes itself and centres on the frames after it opens.
    for _ in 0..4 {
        h.run_ok();
    }
    assert!(h.state().shell().dialog().is_some(), "{name}: a dialog");
    let area = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::unique("ew-dialog")))
        .expect("the dialog's area");
    // A footer wider than the dialog grows to the left of the area's rectangle (the About dialog's four buttons): the
    // footer row's buttons (the lowest row of buttons inside the area, and the buttons on its line) are taken in.
    let all: Vec<egui::Rect> = h.get_all_by_role(egui::Role::Button).map(|n| n.rect()).collect();
    let footer_top = all
        .iter()
        .filter(|r| area.contains_rect(**r))
        .map(|r| r.top())
        .fold(f32::NEG_INFINITY, f32::max);
    let frame = around(
        std::iter::once(area).chain(
            all.into_iter()
                .filter(|r| (r.top() - footer_top).abs() < 1.0 && r.right() > area.left() - 300.0),
        ),
        0.0,
    );
    let image = offscreen::render(dev, &h.ctx, h.output(), recorder);
    record(name, &crop(&image, frame.expand(24.0)), true)
}

/// The last change of the recovery fixture's journal: 2026-10-28 12:00 UTC, in epoch milliseconds.
const FIXTURE_LAST_CHANGE_MS: u64 = 1_793_188_800_000;

/// The recovery dialog's now in the fixture: two hours after [`FIXTURE_LAST_CHANGE_MS`].
const FIXTURE_NOW_MS: u64 = FIXTURE_LAST_CHANGE_MS + 2 * 3_600_000;

/// Sets the time of every transaction in the journals under `dir` to `t` (epoch milliseconds): a journal's `tx` line
/// carries the wall clock of its write, and the recovery dialog shows the newest.
fn pin_journal_times(dir: &std::path::Path, t: u64) {
    const KEY: &str = r#","t":"#;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            pin_journal_times(&path, t);
            continue;
        }
        let in_journal = path.components().any(|c| c.as_os_str() == "journal");
        if !in_journal || path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let mut pinned = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            let at = if line.starts_with(r#"{"j":"tx","#) {
                line.find(KEY)
            } else {
                None
            };
            match at {
                Some(at) => {
                    let digits = at + KEY.len();
                    let end = digits
                        + line[digits..]
                            .find(|c: char| !c.is_ascii_digit())
                            .expect("a value after the time");
                    pinned.push_str(&line[..digits]);
                    pinned.push_str(&t.to_string());
                    pinned.push_str(&line[end..]);
                }
                None => pinned.push_str(line),
            }
        }
        std::fs::write(&path, pinned).unwrap();
    }
}

/// CP6 (FT-571): the four dialogs that need a fixture, each cropped to the dialog's frame with its shadow, light
/// Comfortable, gated: the recovery dialog (an earlier session's journal on a copy of Golden A, its last change
/// pinned to 2026-10-28 12:00 UTC and the dialog's clock to two hours later), the question of discarding that journal
/// (Discard… of the recovery dialog), the check-scheme question (Apply the check scheme of the finished
/// company-standard wizard) and the purge question (a copy of Golden A whose first new logo was replaced and saved,
/// re-opened).
#[test]
fn cp6_the_fixture_dialogs_in_light() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        // The recovery dialog and the question of discarding the journal.
        let tmp = Tmp::new();
        let dir = tmp.golden("g.ew");
        {
            let mut crashed = ShellModel::new(Mode::Gui, tmp.config());
            for (action, params) in [
                ("file.open", json!({ "dir": dir.to_string_lossy() })),
                (
                    "command.run",
                    json!({"cmd": "page.create", "params": {"number": 996}}),
                ),
            ] {
                assert_eq!(crashed.dispatch(action, &params), Outcome::Done, "{action}");
            }
            // A dirty end without a clean close: the journal stays.
            crashed.shutdown();
        }
        pin_journal_times(&tmp.0, FIXTURE_LAST_CHANGE_MS);
        let (mut h, recorder) = window_on(tmp.config(), dir, theme, "comfortable", 1.0, false);
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        ew_ui::set_now_for_tests(&h.ctx, FIXTURE_NOW_MS);
        assert!(
            matches!(
                h.state().shell().dialog(),
                Some(Dialog::Recovery {
                    last_change_ms: FIXTURE_LAST_CHANGE_MS,
                    ..
                })
            ),
            "{:?}",
            h.state().shell().dialog()
        );
        failures.extend(shoot_dialog(
            &dev,
            &mut h,
            &recorder,
            &format!("cp6-dialog-recovery-{theme}"),
        ));
        assert!(h.query_by_label("Last change: 2 hours ago").is_some());
        assert!(h.query_by_label_contains("2026-10-28 12:00 UTC").is_some());
        assert_eq!(
            h.state_mut()
                .shell_mut()
                .dispatch("file.discard_journal", &json!({})),
            Outcome::Done
        );
        assert_eq!(h.state().shell().dialog(), Some(&Dialog::ConfirmJournalDiscard));
        failures.extend(shoot_dialog(
            &dev,
            &mut h,
            &recorder,
            &format!("cp6-dialog-journal-discard-{theme}"),
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);

        // The check-scheme question.
        let tmp = Tmp::new();
        let dir = tmp.golden("g.ew");
        // The wizard previews its draft on the sample project of the resources.
        let (mut h, recorder) = window_on(tmp.config_with_resources(), dir, theme, "comfortable", 1.0, true);
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        let basic = tmp.0.join("company.ew");
        assert_eq!(
            h.state_mut()
                .shell_mut()
                .dispatch("tools.company_standard", &json!({})),
            Outcome::Done
        );
        // The wizard's preview arrives from its worker: on show before the dialog opens over it.
        support::wait_for_preview(&mut h);
        for (action, params) in [
            ("wizard.set", json!({"field": "check_scheme", "value": "full"})),
            ("wizard.finish", json!({ "dir": basic.to_string_lossy() })),
            ("wizard.apply_check_scheme", json!({})),
        ] {
            assert_eq!(
                h.state_mut().shell_mut().dispatch(action, &params),
                Outcome::Done,
                "{action}"
            );
        }
        assert_eq!(
            h.state().shell().dialog(),
            Some(&Dialog::ConfirmCheckScheme {
                scheme: "full".into()
            })
        );
        failures.extend(shoot_dialog(
            &dev,
            &mut h,
            &recorder,
            &format!("cp6-dialog-check-scheme-{theme}"),
        ));
        let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!(null));
        let _ = h.state_mut().shell_mut().dispatch("wizard.cancel", &json!({}));
        h.run_ok();
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);

        // The purge question: the first logo is unused once the project is re-opened (the undo history named it).
        let tmp = Tmp::new();
        let dir = tmp.golden("g.ew");
        {
            let mut shell = ShellModel::new(Mode::Gui, tmp.config());
            let logo = |svg: &str| {
                json!({"cmd": "project.company.set",
                    "params": {"logo": ew_ui::ew_viewmodel::ew_api::base64_encode(svg.as_bytes())}})
            };
            for (action, params) in [
                ("file.open", json!({ "dir": dir.to_string_lossy() })),
                (
                    "command.run",
                    logo(r#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#),
                ),
                ("file.save", json!({})),
                (
                    "command.run",
                    logo(r#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#),
                ),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            shell.shutdown();
        }
        let (mut h, recorder) = window_on(tmp.config(), dir, theme, "comfortable", 1.0, true);
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        assert_eq!(
            h.state_mut()
                .shell_mut()
                .dispatch("project.purge_assets", &json!({})),
            Outcome::Done
        );
        assert!(matches!(
            h.state().shell().dialog(),
            Some(Dialog::ConfirmPurge { .. })
        ));
        failures.extend(shoot_dialog(
            &dev,
            &mut h,
            &recorder,
            &format!("cp6-dialog-purge-{theme}"),
        ));
        let _ = h.state_mut().shell_mut().dispatch("ui.dismiss", &json!(null));
        h.run_ok();
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The rectangles of the buttons named one of `labels`.
fn buttons(h: &Harness<'_, App>, labels: &[&str]) -> Vec<egui::Rect> {
    use egui_kittest::kittest::NodeT as _;
    h.get_all_by_role(egui::Role::Button)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| labels.contains(&l.as_str()))
        })
        .map(|n| n.rect())
        .collect()
}

/// CP9 (FT-533): the quick-fix card in the Messages pane (EW-CORE-023's fix on folio 102, cropped to the pane with the
/// card) and the table card in the Terminal strips pane (a paste onto a read-only cell, cropped to the grid with the
/// card), light Comfortable, gated.
#[test]
fn cp9_the_quick_fix_card_and_the_table_card_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        // The quick-fix card.
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        assert_eq!(
            shell.dispatch("page.goto", &json!({"folio": "102"})),
            Outcome::Done
        );
        let page = shell
            .page_rows()
            .iter()
            .find(|r| r.folio == "102")
            .map(|r| r.id.to_string())
            .expect("folio 102");
        for (action, params) in [
            (
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page],
                    "path": "description_override", "value": "Overridden"}}),
            ),
            ("messages.show", json!({})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        let key = shell
            .nav(ew_ui::ew_viewmodel::navigators::NavKind::Messages)
            .and_then(|n| n.table())
            .and_then(|t| {
                t.nodes
                    .iter()
                    // the page override's fix, not the rebind of EW-PLC-012 (Golden A's EF-02, WP-S1.21 part 3)
                    .find(|n| n.badges.iter().any(|b| b == "fix") && !n.label.contains("drives solenoid"))
                    .map(|n| n.key.clone())
            })
            .expect("a finding with a fix");
        // Every card opens the Preview tab (closing it cancels the card), where the inline card stands as its strip:
        // the Messages tab brought back to the front shows the card's strip.
        assert_eq!(
            shell.dispatch("messages.fix", &json!({"key": key})),
            Outcome::Done
        );
        support::settle(&mut h);
        h.get_by_role_and_label(egui::Role::Button, "Messages").click();
        support::settle(&mut h);
        // The pointer leaves the window so that no hover is painted.
        h.input_mut().events.push(egui::Event::PointerGone);
        dismiss_toasts(&mut h);
        support::wait_for_page(&mut h);
        h.run_ok();
        h.run_ok();
        assert!(
            h.state().shell().properties().card().is_some(),
            "the quick-fix card"
        );
        // FT-567: the "Errors" group title drawn in the accent is the cursor row (a cursor row's title takes the
        // selection colour, as the Pages list's), not a group title in the wrong colour.
        let cursor = h
            .state()
            .shell()
            .nav_cursor(ew_ui::ew_viewmodel::navigators::NavKind::Messages);
        assert!(
            cursor.as_deref().is_some_and(|k| k.ends_with("severity:error")),
            "the Messages cursor is on the Errors group: {cursor:?}"
        );
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let mut parts = vec![messages_pane(&h)];
        parts.extend(buttons(&h, &["Apply", "Cancel"]));
        failures.extend(record(
            &format!("cp9-fix-card-{theme}"),
            &crop(&image, around(parts, 4.0)),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);

        // The table card.
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("props.show", json!({"on": false})),
            ("navigator.show", json!({"kind": "strips"})),
            ("navigator.table", json!({"kind": "strips"})),
            ("navigator.filter", json!({"kind": "strips", "text": "x1032"})),
            ("navigator.sort", json!({"kind": "strips", "by": "terminal"})),
            ("ui.focus", json!({"area": "navigator", "kind": "strips"})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        support::settle(&mut h);
        h.event(egui::Event::Paste("X".into()));
        support::settle(&mut h);
        assert!(h.state().shell().table_gesture().is_some(), "the table card");
        dismiss_toasts(&mut h);
        h.run_ok();
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let grid = node_rect(&h, "Terminal strips table").expect("the grid");
        let top = node_rect(&h, "Table").map_or(grid.top(), |r| r.top());
        let mut parts = vec![egui::Rect::from_min_max(egui::pos2(grid.left(), top), grid.max)];
        // The card's buttons, not the Preview tab's.
        parts.extend(
            buttons(&h, &["Apply", "Cancel", "Drop"])
                .into_iter()
                .filter(|r| r.right() <= grid.right()),
        );
        failures.extend(record(
            &format!("cp9-table-card-{theme}"),
            &crop(&image, around(parts, 4.0)),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Doc 16a §6.4 (FT-540's restyle): the bottom dock with Messages and History open, History active, cropped to the
/// dock's tab bar, light Comfortable, gated.
#[test]
fn the_bottom_dock_tab_bar_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        for action in ["messages.show", "history.show"] {
            assert_eq!(shell.dispatch(action, &json!({})), Outcome::Done, "{action}");
        }
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let pane = h
            .get_all_by_role(egui::Role::Group)
            .find(|n| {
                use egui_kittest::kittest::NodeT as _;
                n.accesskit_node().label().as_deref() == Some("History")
            })
            .map(|n| n.rect())
            .expect("the History panel");
        let tabs: Vec<egui::Rect> = h
            .get_all_by_role(egui::Role::Button)
            .filter_map(|n| {
                use egui_kittest::kittest::NodeT as _;
                let label = n.accesskit_node().label()?;
                let r = n.rect();
                ((label.starts_with("Messages") || label.starts_with("History"))
                    && r.bottom() <= pane.top() + 1.0
                    && r.top() >= pane.top() - 60.0)
                    .then_some(r)
            })
            .collect();
        assert!(tabs.len() >= 2, "two dock tabs: {tabs:?}");
        let bar = around(tabs, 0.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(pane.left(), bar.top()),
            egui::pos2(pane.right(), bar.bottom() + 1.0),
        );
        let strip = crop(&image, bar);
        // The bar's top rows between the tabs and the right-hand buttons are its plain fill: no band from the page
        // view's floating HUD (its shadow used to spill onto the bar, cycle 33).
        let at = |x: u32, y: u32| {
            let i = ((y * strip.width + x) * 4) as usize;
            strip.pixels[i..i + 3].to_vec()
        };
        let fill = at(strip.width / 2, strip.height / 2);
        for y in 0..3 {
            for x in strip.width / 5..strip.width * 4 / 5 {
                assert_eq!(at(x, y), fill, "{theme}: the tab bar's fill at ({x}, {y})");
            }
        }
        failures.extend(record(&format!("dock-tabs-{theme}-comfortable"), &strip, true));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Doc 16a §6.24 (FT-531's restyle): the chip line "Not on this page" with its pill jump chips, for -201U3 selected on
/// folio 102 (its other representations are on 201 and 203), cropped to the chip line, light Comfortable, gated.
#[test]
fn the_chip_line_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("navigator.show", json!({"kind": "devices"})),
            ("page.goto", json!({"folio": "102"})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        support::wait_for_page(&mut h);
        // -201U3 is under -201U1: revealed by its row, then marked (the navigator's selection makes the chips).
        let shell = h.state_mut().shell_mut();
        let key = shell
            .nav(ew_ui::ew_viewmodel::navigators::NavKind::Devices)
            .and_then(|n| n.table())
            .and_then(|t| {
                t.nodes
                    .iter()
                    .find(|n| {
                        n.role == ew_ui::ew_viewmodel::navigators::NavRole::Device && n.label == "-201U3"
                    })
                    .map(|n| n.key.clone())
            })
            .expect("the -201U3 row");
        for (action, params) in [
            ("navigator.row", json!({"kind": "devices", "key": key})),
            ("navigator.select", json!({"kind": "devices", "keys": [key]})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        h.run_ok();
        assert!(!h.state().shell().chips().is_empty(), "chips for -201U3");
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let mut parts = vec![node_rect(&h, "Not on this page:").expect("the chip line's label")];
        parts.extend(h.get_all_by_role(egui::Role::Button).filter_map(|n| {
            use egui_kittest::kittest::NodeT as _;
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Open "))
                .then(|| n.rect())
        }));
        failures.extend(record(
            &format!("chip-line-{theme}-comfortable"),
            &crop(&image, around(parts, 4.0)),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Dispatches `action` with `params` on the window's shell; it must be done.
fn act(h: &mut Harness<'_, App>, action: &str, params: serde_json::Value) {
    let outcome = h.state_mut().shell_mut().dispatch(action, &params);
    assert_eq!(outcome, Outcome::Done, "{action} {params}");
}

/// The page point and the sheet point of grid index `k` on the focused page.
fn grid_points(h: &Harness<'_, App>, k: (i64, i64)) -> ((i64, i64), (i64, i64)) {
    let sn = &h.state().shell().canvas().unwrap().page().snapper;
    let p = (sn.page_of_index(k.0), sn.page_of_index(k.1));
    (p, sn.page_to_sheet(p))
}

/// A press and a release with Alt (no autoconnect wires) at grid index `k`.
fn place_at(h: &mut Harness<'_, App>, k: (i64, i64)) {
    let (_, s) = grid_points(h, k);
    let at = json!({"x_mm": s.0 as f64 / 1e6, "y_mm": s.1 as f64 / 1e6, "alt": true});
    act(h, "canvas.press", at.clone());
    act(h, "canvas.release", at);
}

/// The representation at grid index `k`: its ID and the tag it prints.
fn rep_at(h: &Harness<'_, App>, k: (i64, i64)) -> (String, String) {
    let (p, _) = grid_points(h, k);
    let t = std::sync::Arc::clone(&h.state().shell().canvas().unwrap().page().tables);
    let rep = t.reps.iter().find(|r| r.pos == p).expect("a representation");
    let label = &t.cps.iter().find(|c| c.rep == rep.id).unwrap().label;
    (rep.id.to_string(), label.rsplit_once(':').unwrap().0.to_owned())
}

/// Doc 16a §6.22, second use (WP-S2.18 D2): the Place tool holding a NO contact on folio 101 with the Join chooser
/// open: "New device", a slot row ("slot 13-14", a relay whose contact was deleted) and the relays as "auxiliary"
/// rows, the slot row active; cropped to the popup with its shadow, light Comfortable, gated.
#[test]
fn the_join_chooser_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        // Two relays on the empty lower right of folio 101; the second gets a slot (a contact placed, then deleted).
        for k in [(60, 30), (60, 40)] {
            act(
                &mut h,
                "place.pick",
                json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
            );
            place_at(&mut h, k);
            act(&mut h, "ui.dismiss", json!({}));
        }
        let (_, k2) = rep_at(&h, (60, 40));
        act(
            &mut h,
            "place.pick",
            json!({"symbol": "ew.iec.contact-no@qet10", "device": k2}),
        );
        place_at(&mut h, (66, 40));
        act(&mut h, "ui.dismiss", json!({}));
        let (contact, _) = rep_at(&h, (66, 40));
        act(&mut h, "select.set", json!({"ids": [contact]}));
        act(&mut h, "edit.delete_selection", json!({}));
        act(&mut h, "ui.dismiss", json!({}));
        act(
            &mut h,
            "place.pick",
            json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
        );
        act(&mut h, "place.join", json!({}));
        act(&mut h, "chooser.row", json!({"step": 1}));
        // The page image of the edited page behind the popup's shadow (the render worker is slower under load).
        support::wait_for_page(&mut h);
        let ch = h.state().shell().chooser().expect("the chooser").clone();
        assert_eq!(ch.rows[0].name, "New device");
        assert_eq!((ch.row, ch.rows[1].kind.as_str()), (1, "slot 13-14"));
        assert!(ch.rows.iter().any(|r| r.kind == "auxiliary"), "{:?}", ch.rows);
        // The active row's object, -K2's coil, has the hover outline (§6.22, §7.1).
        let (coil, _) = rep_at(&h, (60, 40));
        let shell = h.state().shell();
        let c = shell.canvas().unwrap();
        let outline = c
            .page()
            .entities
            .iter()
            .find(|(e, _)| e.0.to_string() == coil)
            .map(|(_, r)| *r);
        assert_eq!(ew_ui::canvas::hover_rect(shell, 0, c), outline);
        assert!(outline.is_some());
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        let area = egui::Id::unique("join-chooser").with("area");
        let popup = h.ctx.memory(|m| m.area_rect(area)).expect("the chooser's popup");
        // The popup's shadow (elevation.1) spills a few points around its frame.
        failures.extend(record(
            &format!("join-chooser-{theme}-comfortable"),
            &crop(&image, popup.expand(16.0)),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Cycle 36 (window check W36, doc 16a §6.8): the Properties header of a changeover contact of a relay placed from
/// EW-GEN-RLY-1 on folio 101, whose open findings are more than two: one count chip per severity at the right, the title kept whole at the left,
/// the filter and the rows under it in place; cropped to the header and the first rows, light Comfortable, gated.
#[test]
fn the_properties_header_with_many_findings_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        act(&mut h, "place.pick", json!({"part": "EW-GEN-RLY-1"}));
        place_at(&mut h, (60, 30));
        act(&mut h, "ui.dismiss", json!({}));
        let (_, tag) = rep_at(&h, (60, 30));
        // A changeover contact of the relay, its three points open: three warnings beside the coil's two.
        act(
            &mut h,
            "place.pick",
            json!({"symbol": "ew.iec.contact-co@qet10", "device": tag}),
        );
        place_at(&mut h, (68, 30));
        act(&mut h, "ui.dismiss", json!({}));
        let (contact, _) = rep_at(&h, (68, 30));
        act(&mut h, "select.set", json!({"ids": [contact]}));
        act(&mut h, "props.show", json!({"on": true}));
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        support::wait_for_page(&mut h);
        h.run_ok();
        let findings = {
            let shell = h.state().shell();
            let sheet = shell.properties().sheet().expect("a sheet");
            assert_eq!(sheet.header.title, tag);
            sheet.header.findings.clone()
        };
        assert!(findings.len() > 2, "{findings:?}");
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        // One chip per severity, named "<n> <Words>: <rule>: <message>; …", and the title's own node.
        let chips: Vec<String> = {
            use egui_kittest::kittest::NodeT as _;
            h.root()
                .children_recursive()
                .filter_map(|n| {
                    let a = n.accesskit_node();
                    a.label().or_else(|| a.value())
                })
                .filter(|l| l.contains(" Warnings: ") || l.contains(" Errors: "))
                .collect()
        };
        assert!(!chips.is_empty() && chips.len() <= 3, "{chips:?}");
        let title = node_rect(&h, &tag).expect("the header's title");
        let right = h
            .get_all_by_role(egui::Role::Splitter)
            .find(|n| {
                use egui_kittest::kittest::NodeT as _;
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.ends_with(" and Page view"))
            })
            .map(|n| n.rect().left())
            .expect("the left column's splitter");
        assert!(
            title.right() <= right,
            "the title stays in the pane: {title:?}, {right}"
        );
        let region = egui::Rect::from_min_max(
            egui::pos2(title.left() - 24.0, title.top() - 40.0),
            egui::pos2(right, title.bottom() + 200.0),
        );
        failures.extend(record(
            &format!("props-header-findings-{theme}-comfortable"),
            &crop(&image, region),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// FT-597 (doc 16a §6.18): the quick-access Save button's tooltip, cropped to the button and the tooltip under it,
/// light Comfortable, gated: the tooltip is drawn on `color.surface.inverse` with `color.text.on_inverse`.
#[test]
fn the_save_tooltip_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        let save = h
            .get_all_by_role(egui::Role::Button)
            .find(|n| {
                use egui_kittest::kittest::NodeT as _;
                n.accesskit_node().label().as_deref() == Some("Save")
            })
            .map(|n| n.rect())
            .expect("the Save button");
        h.hover_at(save.center());
        for _ in 0..60 {
            h.step();
        }
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        // The tooltip's fill beside its text is the inverse surface: dark in light.
        let label = node_rect(&h, "Save (Ctrl+S)").expect("the tooltip's text");
        let fill = luma(&image, (label.left() - 4.0) as u32, label.center().y as u32);
        assert!(fill < 80.0, "light: the tooltip's fill is dark ({fill})");
        let region =
            egui::Rect::from_min_size(save.left_top() - egui::vec2(8.0, 8.0), egui::vec2(360.0, 120.0));
        failures.extend(record(
            &format!("tooltip-save-{theme}"),
            &crop(&image, region),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// FT-625: four window details, each in its own window on a fresh copy of Golden A (their
/// edits would show in each other's crops), light Comfortable, gated.
#[test]
fn ft625_the_window_details_in_light() {
    let dev = device();
    let mut failures = Vec::new();
    ft625_the_place_ghost_wires_and_hud_hint(&dev, &mut failures);
    ft625_the_counted_toasts(&dev, &mut failures);
    ft625_the_wire_marker_row(&dev, &mut failures);
    ft625_the_potential_name_cell(&dev, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// FT-625 (FT-545): the Place tool holding a NO contact between two contacts on folio 101, its ghost facing both
/// with the wires autoconnect will draw, and the HUD saying how many and how to place without them; cropped to the
/// page pane, light Comfortable.
fn ft625_the_place_ghost_wires_and_hud_hint(
    dev: &ew_render2d::gpu::HeadlessDevice,
    failures: &mut Vec<String>,
) {
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let pick = json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"});
        // Two contacts one above the other without wires (as `gpp_m34`'s FT-545 test places them).
        for k in [(60, 38), (60, 52)] {
            act(&mut h, "place.pick", pick.clone());
            place_at(&mut h, k);
            act(&mut h, "ui.dismiss", json!({}));
        }
        act(&mut h, "place.pick", pick);
        let (_, s) = grid_points(&h, (60, 43));
        act(
            &mut h,
            "cursor.set",
            json!({"x_mm": s.0 as f64 / 1e6, "y_mm": s.1 as f64 / 1e6}),
        );
        let hud = ew_ui::ew_viewmodel::command_bar::hud(h.state().shell());
        assert!(hud.prompt.contains("places without"), "{}", hud.prompt);
        support::wait_for_page(&mut h);
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        h.run_ok();
        let image = offscreen::render(dev, &h.ctx, h.output(), &recorder);
        let (_, hud_rect, pane) = h.state().command_surface_rects();
        let (hud_rect, pane) = (hud_rect.expect("the HUD"), pane.expect("the page pane"));
        assert!(pane.contains_rect(hud_rect), "the HUD in the page pane");
        failures.extend(record(
            &format!("ft625-place-wires-{theme}-comfortable"),
            &crop(&image, pane),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
}

/// FT-625 (FT-549, FT-554): the counted toasts one after the other, Run checks by severity (`check`) and the
/// marker-mode switch's count (`marker`), at the canvas's top right; cropped to the toast with its shadow, light
/// Comfortable.
fn ft625_the_counted_toasts(dev: &ew_render2d::gpu::HeadlessDevice, failures: &mut Vec<String>) {
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        for (name, action, params, text) in [
            ("check", "messages.check", json!({}), "Check: "),
            (
                "marker",
                "project.marker_mode",
                json!({"mode": "explicit"}),
                "10 wires carry a marker",
            ),
        ] {
            dismiss_toasts(&mut h);
            act(&mut h, action, params);
            let toast = h.state().shell().toast().expect("a toast").text.clone();
            assert!(toast.starts_with(text), "{toast}");
            h.run_ok();
            h.run_ok();
            let image = offscreen::render(dev, &h.ctx, h.output(), &recorder);
            // The toasts' area (`ew_ui`'s `notifications::TOASTS_AREA`, as `tests/part4_cards.rs` names it).
            let area = egui::Id::unique("ew-toasts");
            let toasts = h.ctx.memory(|m| m.area_rect(area)).expect("the toasts' area");
            failures.extend(record(
                &format!("ft625-toast-{name}-{theme}-comfortable"),
                &crop(&image, toasts.expand(16.0)),
                true,
            ));
        }
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
}

/// FT-625 (FT-554): Properties of the wire W29 on folio 103, the Main group with "Wire marker" right after the
/// cross-section and the colour; cropped to those rows in the Properties pane, light Comfortable.
fn ft625_the_wire_marker_row(dev: &ew_render2d::gpu::HeadlessDevice, failures: &mut Vec<String>) {
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        act(&mut h, "page.goto", json!({"folio": "103"}));
        support::wait_for_page(&mut h);
        act(
            &mut h,
            "select.set",
            json!({"ids": ["9eb2d865-fca4-59b1-87bf-c073d7190ceb"]}),
        );
        act(&mut h, "props.show", json!({"on": true}));
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        support::wait_for_page(&mut h);
        h.run_ok();
        let image = offscreen::render(dev, &h.ctx, h.output(), &recorder);
        let row = node_rect(&h, "Wire marker").expect("the Wire marker row");
        let first = node_rect(&h, "Cross-section").expect("the cross-section row");
        // The left column's right edge: its splitter with the page view.
        let right = h
            .get_all_by_role(egui::Role::Splitter)
            .find(|n| {
                use egui_kittest::kittest::NodeT as _;
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.ends_with(" and Page view"))
            })
            .map(|n| n.rect().left())
            .expect("the left column's splitter");
        let region = egui::Rect::from_min_max(
            egui::pos2(row.left() - 12.0, first.top() - 40.0),
            egui::pos2(right, row.bottom() + 40.0),
        );
        failures.extend(record(
            &format!("ft625-wire-marker-{theme}-comfortable"),
            &crop(&image, region),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
}

/// FT-625 (FT-577): the Potentials table with a potential known only at its connection points (24VDC-4's definition
/// deleted), its Name cell in the editor with a typed name; cropped to the grid, light
/// Comfortable.
fn ft625_the_potential_name_cell(dev: &ew_render2d::gpu::HeadlessDevice, failures: &mut Vec<String>) {
    use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
    {
        let theme = "light";
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        // The editor's caret blinks on the wall clock: steady for a stable image.
        h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
        act(&mut h, "props.show", json!({"on": false}));
        act(&mut h, "navigator.show", json!({"kind": "potentials"}));
        act(&mut h, "navigator.table", json!({"kind": "potentials"}));
        let def = {
            let shell = h.state().shell();
            let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Potential && n.label == "24VDC-4")
                .map(|n| n.records[0])
                .expect("24VDC-4")
        };
        act(
            &mut h,
            "command.run",
            json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "potential_def", "id": def.to_string()}]}}),
        );
        let (key, label) = {
            let shell = h.state().shell();
            let t = shell.nav(NavKind::Potentials).and_then(|n| n.table()).unwrap();
            let n = t
                .nodes
                .iter()
                .find(|n| n.table_row && n.kinds == ["cp"])
                .expect("a potential known only at a CP");
            (n.key.clone(), n.label.clone())
        };
        act(
            &mut h,
            "navigator.filter",
            json!({"kind": "potentials", "text": label}),
        );
        act(
            &mut h,
            "ui.focus",
            json!({"area": "navigator", "kind": "potentials"}),
        );
        act(
            &mut h,
            "table.edit",
            json!({"kind": "potentials", "row": key, "column": "potential"}),
        );
        act(
            &mut h,
            "table.input",
            json!({"kind": "potentials", "text": "24VDC-9"}),
        );
        dismiss_toasts(&mut h);
        support::settle(&mut h);
        h.run_ok();
        let image = offscreen::render(dev, &h.ctx, h.output(), &recorder);
        let grid = node_rect(&h, "Potentials table").expect("the grid");
        let top = node_rect(&h, "Table").map_or(grid.top(), |r| r.top());
        // The tool line, the header and the two rows the filter leaves (the empty grid below them is cut).
        let pane = egui::Rect::from_min_max(
            egui::pos2(grid.left(), top - 4.0),
            egui::pos2(grid.right(), grid.bottom().min(top + 140.0)),
        );
        failures.extend(record(
            &format!("ft625-potential-name-{theme}-comfortable"),
            &crop(&image, pane),
            true,
        ));
        h.state_mut().shell_mut().shutdown();
        drop(h);
        drop(tmp);
    }
}

/// FT-675: folio 103 of Golden A without its bond B1 (0 V and PE merge over W27 and EW-POT-001 stands): the CPU
/// snapshot of the page pane (800 x 560 points at ppp 1, the page fitted) with the merged potential's wires drawn over in
/// `canvas.conflict`; the pixel on the middle of a conflict segment is that colour.
#[test]
fn ft675_the_conflict_overlay_in_light() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    let run = |shell: &mut ShellModel, action: &str, params: serde_json::Value| {
        assert_eq!(
            shell.dispatch(action, &params),
            Outcome::Done,
            "{action} {params}"
        );
    };
    run(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    run(&mut shell, "page.goto", json!({"folio": "103"}));
    run(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let bond = {
        let t = shell
            .nav(ew_ui::ew_viewmodel::navigators::NavKind::Potentials)
            .and_then(|n| n.table())
            .expect("the Potentials table");
        let rows: Vec<_> = t.nodes.iter().filter(|n| n.kinds == ["bond"]).collect();
        assert_eq!(rows.len(), 1, "Golden A has the one bond B1");
        rows[0].records[0]
    };
    run(
        &mut shell,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "bond", "id": bond.to_string()}]}}),
    );
    run(
        &mut shell,
        "navigator.show",
        json!({"kind": "potentials", "on": false}),
    );
    run(
        &mut shell,
        "view.canvas_resize",
        json!({"width": 800.0, "height": 560.0, "ppp": 1.0}),
    );
    run(&mut shell, "view.fit_page", json!({}));
    let lines = shell.conflict_overlay(0);
    assert!(!lines.is_empty(), "the merged potential's wires on 103");
    let (image, _) = ew_ui::canvas::snapshot_image(&shell, 0).expect("a snapshot");
    // The middle of the longest conflict segment is `canvas.conflict` (#C42B3C).
    let c = shell.canvas().expect("the canvas");
    let view = ew_render2d::view::View {
        centre: c.centre(),
        zoom: c.zoom(),
        pane: c.native_pane(),
        ppp: c.native_ppp(),
    };
    let t = view.settled_transform();
    let (a, b) = lines
        .iter()
        .flat_map(|l| l.windows(2).map(|s| (s[0], s[1])))
        .max_by_key(|(a, b)| (a.0 - b.0).abs() + (a.1 - b.1).abs())
        .expect("a segment");
    let mid = t.to_screen(((a.0 + b.0) as f64 / 2.0, (a.1 + b.1) as f64 / 2.0));
    let i = (mid.1 as usize * image.width as usize + mid.0 as usize) * 4;
    assert_eq!(
        &image.pixels[i..i + 3],
        &[0xC4, 0x2B, 0x3C],
        "the conflict colour at {mid:?}"
    );
    let failures: Vec<String> = record("ft675-conflict-overlay-103-light", &image, true)
        .into_iter()
        .collect();
    shell.shutdown();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
