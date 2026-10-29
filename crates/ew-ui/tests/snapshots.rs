// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chrome snapshots of WP-S2.17 (§11 item 3, DL6, DK7): the main window on Golden A folio 101 at 1280 × 800 in
//! every theme and density, and the component gallery at ppp 1 and 2, rendered offscreen (`offscreen`) and written
//! as PNG files under `target/ew-ui-snapshots/` (or `EW_SNAPSHOT_DIR`) for the reviewer, who compares them with the
//! mockup side by side.
//!
//! The pixel gate (D7 as amended: no `egui_kittest` render feature): the main window in light and dark at both
//! densities and the gallery in the four themes at ppp 1 are compared with the blessed PNG files under
//! `tests/expected/snapshots/`. A pixel differs when a channel differs by more than
//! [`offscreen::CHANNEL_TOLERANCE`]; an image fails with more than [`offscreen::MAX_DIFFERING_PIXELS`] differing
//! pixels, and a diff image (differences in red) is written next to the output. `EW_BLESS=1` writes the blessed
//! files. Behind the `snapshots` feature and on Windows only (DX12 WARP: one platform keeps the pixels stable); no
//! adapter there is a failure.

#![cfg(all(feature = "snapshots", windows))]

mod gallery;
mod offscreen;
mod support;

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::{App, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
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
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
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
    assert_eq!(
        h.state_mut()
            .shell_mut()
            .dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    h.run_ok();
    support::wait_for_page(&mut h);
    h.run_ok();
    (h, recorder, tmp)
}

#[test]
fn the_main_window_in_every_theme_and_density() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for density in ["comfortable", "compact"] {
            let (h, recorder, tmp) = main_window(theme, density);
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            assert_eq!((image.width, image.height), (1280, 800));
            let gated = matches!(theme, "light" | "dark");
            failures.extend(record(&format!("main-{theme}-{density}"), &image, gated));
            // The project closes before its directory is removed.
            drop(h);
            drop(tmp);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// DK7's 300 % variant, for review only (not gated, as the gallery at ppp 2): the main window, light and Comfortable,
/// at 1280 × 800 points on a 300 % display (3840 × 2400 pixels). The setting `ui_scale` stops at 200 %
/// (`UI_SCALES`), so 300 % is the system's scale (egui's native pixels per point), which is how a 300 % display
/// reaches the window. Not gated: a 3840 × 2400 blessed image would be re-blessed with every chrome change.
#[test]
fn the_main_window_at_300_percent_for_review() {
    let dev = device();
    let (h, recorder, tmp) = main_window_at("light", "comfortable", 3.0);
    let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
    assert_eq!((image.width, image.height), (3840, 2400));
    assert_eq!(record("main-light-comfortable-300", &image, false), None);
    drop(h);
    drop(tmp);
}

/// For review only (not gated): the Devices navigator in table mode and the history panel, light and dark.
#[test]
fn the_table_and_the_history_panel_for_review() {
    let dev = device();
    for theme in ["light", "dark"] {
        let (mut h, recorder, tmp) = main_window(theme, "comfortable");
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("navigator.show", json!({"kind": "devices"})),
            ("navigator.table", json!({"kind": "devices"})),
            ("history.show", json!({})),
        ] {
            assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
        }
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
        assert_eq!(record(&format!("table-history-{theme}"), &image, false), None);
        drop(h);
        drop(tmp);
    }
}

#[test]
fn the_component_gallery_in_every_theme_at_ppp_1_and_2() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark", "hc-dark", "hc-light"] {
        for (ppp, gated) in [(1.0_f32, true), (2.0, false)] {
            let recorder = offscreen::Recorder::default();
            let mut h = Harness::builder()
                .with_size(Vec2::new(640.0, 560.0))
                .with_pixels_per_point(ppp)
                .with_context_setup(setup)
                .renderer(recorder.clone())
                .build_ui_state(move |ui, _: &mut ()| gallery::show(ui, theme), ());
            h.run_ok();
            let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
            failures.extend(record(&format!("gallery-{theme}-ppp{ppp}"), &image, gated));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `image`'s pixels inside `rect` (points at ppp 1), clamped to the image.
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

/// CB7 (§12.2): the command bar alone, cropped to its frame, for each of the six tabs, in light comfortable and dark
/// compact (gated).
#[test]
fn the_command_bar_on_every_tab() {
    let dev = device();
    let mut failures = Vec::new();
    for (theme, density) in [("light", "comfortable"), ("dark", "compact")] {
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
        let _ = shell.dispatch("file.close", &json!({}));
        h.run_ok();
        h.run_ok();
        let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
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
/// cursor on the pending finding with its detail block) and the History panel (three steps, a save marker, one undone), each cropped to its pane, in light
/// and dark at both densities, gated.
#[test]
fn dk12_messages_and_history_in_light_and_dark_at_both_densities() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark"] {
        for density in ["comfortable", "compact"] {
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
                let image = offscreen::render(&dev, &h.ctx, h.output(), &recorder);
                failures.extend(record(
                    &format!("dk12-{name}-{theme}-{density}"),
                    &crop(&image, messages_pane(&h)),
                    true,
                ));
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
/// Cropped to the pane from its tool line to the grid's bottom, light and dark at both densities, gated.
#[test]
fn dk12_the_data_grid_in_light_and_dark_at_both_densities() {
    let dev = device();
    let mut failures = Vec::new();
    for theme in ["light", "dark"] {
        for density in ["comfortable", "compact"] {
            let (mut h, recorder, tmp) = main_window(theme, density);
            // The editor's caret blinks on the wall clock: steady for a stable image.
            h.ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
            let shell = h.state_mut().shell_mut();
            for (action, params) in [
                ("props.show", json!({"on": false})),
                ("navigator.show", json!({"kind": "strips"})),
                ("navigator.table", json!({"kind": "strips"})),
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
