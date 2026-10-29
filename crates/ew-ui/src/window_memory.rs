// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Window memory in the window (WP-S2.17 part 3 item 9, DK6; FT-348): the window builder from the saved geometry, and
//! the geometry the running window reports, recorded in the shell's settings (written once, at the end of the window,
//! as a merge of `[window]` into the settings file on disk: `ew_viewmodel::settings::Settings::save_window`).
//!
//! Only a real window records: egui reports the outer rectangle and the monitor's size only there (a test harness
//! reports neither), so a test's settings file keeps its bytes. A minimised or full-screen window records nothing; a
//! maximised one keeps its last normal size and position with `maximized = true`.
//!
//! **Units.** The size is in points at the zoom 1, the position in physical pixels of the desktop (R2-1).
//!
//! - *The size.* egui reports the window's rectangles in its points at the current zoom (the physical pixels divided
//!   by the native scale times egui's zoom factor, which is the UI scale); the recorded size is the reported one times
//!   the zoom factor of the frame the report was made with, the logical size. eframe opens the window at the zoom 1,
//!   so the builder takes it back, and the system keeps a window's logical size when it moves to a monitor of another
//!   scale: the size survives every UI scale and every monitor scale.
//! - *The position* is the reported outer corner times the zoom and the native scale of that report: physical pixels,
//!   one space for every monitor. A logical position does not survive a desktop whose monitors have different scales:
//!   it is divided by the scale of the monitor the window was on and multiplied back by the scale of the monitor the
//!   window is created on, so the window drifted at every start (2200 → 3300 → 4950 px with a 150 % primary and the
//!   window on a 100 % secondary). The builder gets the saved position only as a first guess; the first frame moves
//!   the window to the saved physical position ([`first_frame_position`]) and runs the clamp there.
//! - A frame's report is made before the frame applies a new zoom factor (egui applies
//!   [`egui::Context::set_zoom_factor`] at the start of the next pass), so a frame whose zoom differs from the previous
//!   frame's records nothing.
//!
//! `[window] x/y` were points before R2-1 (cycle 32, never released); such a value is read as pixels, and the clamp
//! of the first frame keeps the window on a monitor whatever it is.
//!
//! **The clamp at start** (FT-526). eframe 0.37 without its `persistence` feature knows no monitor before the window
//! opens, so the builder gets the saved geometry with the size bounds of [`ew_viewmodel::window::restore`] only (and
//! eframe's own clamp of the size to the largest monitor). On the first frame the executable reads the monitors from
//! eframe's winit window and [`first_frame_position`] moves the window to its saved position and runs the same pure
//! clamp ([`placement`]) there: a window whose title bar is on no monitor (its monitor gone) moves onto the primary
//! one. A window restored maximised is left where the system maximised it.

use egui::{ViewportBuilder, ViewportInfo};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::window::{self, MonitorArea, WindowGeometry};

/// The window builder `builder` with the saved geometry `saved` restored: its size, its maximised state and its
/// position as a first guess (the saved pixels taken as points; the first frame puts the window at the saved pixels).
pub fn restore(builder: ViewportBuilder, saved: Option<WindowGeometry>) -> ViewportBuilder {
    let Some(g) = saved.map(|g| window::restore(g, &[])) else {
        return builder;
    };
    builder
        .with_inner_size([g.width, g.height])
        .with_position([g.x, g.y])
        .with_maximized(g.maximized)
}

/// Where a window that opened with its outer top-left corner at `at` and the inner size `size` moves to so that it
/// lies on one of the system's `monitors` (the primary first), all in points at the zoom 1; `None` when it stays (its
/// title bar's grip is on a monitor, or no monitor is known). The clamp of [`ew_viewmodel::window::restore`].
pub fn placement(at: (f32, f32), size: (f32, f32), monitors: &[MonitorArea]) -> Option<(f32, f32)> {
    if monitors.is_empty() {
        return None;
    }
    let opened = WindowGeometry {
        x: at.0,
        y: at.1,
        width: size.0,
        height: size.1,
        maximized: false,
    };
    let g = window::restore(opened, monitors);
    ((g.x, g.y) != at).then_some((g.x, g.y))
}

/// The first frame's move of the window (FT-526, R2-1, R2-2): the `OuterPosition` to send, in points at the zoom
/// `zoom`, or `None` when the window stays. The saved outer position `saved_px` (when there is one), the window's outer
/// top-left corner `outer_px` and inner size `inner_px` as it opened, and the system's monitors `monitors_px` (each
/// with whether it is the primary) are physical pixels; `scale` is the window's native scale. The window goes to the
/// saved position, clamped onto a monitor: the clamp runs on one uniform space, the physical pixels over `scale`
/// (monitors of different scales keep their places in it), with the primary monitor first and the others in the
/// system's order. egui-winit turns the result back into physical pixels by multiplying it by `zoom × scale`. Without
/// a monitor nothing moves.
pub fn first_frame_position(
    saved_px: Option<(f32, f32)>,
    outer_px: (f32, f32),
    inner_px: (f32, f32),
    monitors_px: &[(MonitorArea, bool)],
    scale: f32,
    zoom: f32,
) -> Option<egui::Pos2> {
    if !(scale.is_finite() && scale > 0.0 && zoom.is_finite() && zoom > 0.0) {
        return None;
    }
    let monitors: Vec<MonitorArea> = monitors_px
        .iter()
        .filter(|(_, primary)| *primary)
        .chain(monitors_px.iter().filter(|(_, primary)| !*primary))
        .map(|(m, _)| MonitorArea {
            x: m.x / scale,
            y: m.y / scale,
            width: m.width / scale,
            height: m.height / scale,
        })
        .filter(|m| m.width > 0.0 && m.height > 0.0)
        .collect();
    if monitors.is_empty() {
        return None;
    }
    let now = (outer_px.0 / scale, outer_px.1 / scale);
    let want = saved_px
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map_or(now, |(x, y)| (x / scale, y / scale));
    let size = (inner_px.0 / scale, inner_px.1 / scale);
    let to = placement(want, size, &monitors).unwrap_or(want);
    (to != now).then(|| egui::pos2(to.0 / zoom, to.1 / zoom))
}

/// The geometry a running window reports in `info`, its rectangles in points at the zoom factor `zoom` and its native
/// scale, given the last one recorded: the size in points at the zoom 1, the position in physical pixels (see the
/// module documentation); `None` when there is nothing to record (no real window, no native scale, minimised, full
/// screen).
pub fn observed(info: &ViewportInfo, zoom: f32, last: Option<WindowGeometry>) -> Option<WindowGeometry> {
    if info.monitor_size.is_none() || info.minimized == Some(true) || info.fullscreen == Some(true) {
        return None;
    }
    let native = info
        .native_pixels_per_point
        .filter(|s| s.is_finite() && *s > 0.0)?;
    let inner = info.inner_rect? * zoom;
    let outer = info.outer_rect? * (zoom * native);
    if info.maximized == Some(true) {
        // The size and position before it was maximised stay.
        let normal = last.unwrap_or(WindowGeometry {
            x: outer.min.x,
            y: outer.min.y,
            width: inner.width(),
            height: inner.height(),
            maximized: true,
        });
        return Some(WindowGeometry {
            maximized: true,
            ..normal
        });
    }
    Some(WindowGeometry {
        x: outer.min.x,
        y: outer.min.y,
        width: inner.width(),
        height: inner.height(),
        maximized: false,
    })
}

/// Records the geometry of the window of `ctx` in `shell` when it changed. `last_zoom` is the zoom factor of the
/// previous frame (updated here): the viewport report of this frame was made with it, so a frame whose zoom differs
/// records nothing.
pub(crate) fn remember(ctx: &egui::Context, shell: &mut ShellModel, last_zoom: &mut Option<f32>) {
    let zoom = ctx.zoom_factor();
    let stable = *last_zoom == Some(zoom);
    *last_zoom = Some(zoom);
    if !stable {
        return;
    }
    let last = shell.settings().window;
    if let Some(g) = ctx.input(|i| observed(i.viewport(), zoom, last))
        && Some(g) != last
    {
        shell.set_window(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, pos2, vec2};

    fn info(maximized: bool) -> ViewportInfo {
        ViewportInfo {
            monitor_size: Some(vec2(1920.0, 1080.0)),
            outer_rect: Some(Rect::from_min_size(pos2(100.0, 50.0), vec2(1296.0, 839.0))),
            inner_rect: Some(Rect::from_min_size(pos2(108.0, 81.0), vec2(1280.0, 800.0))),
            maximized: Some(maximized),
            native_pixels_per_point: Some(1.0),
            ..ViewportInfo::default()
        }
    }

    /// DK6: a normal window records its outer position and inner size; a maximised one keeps the last normal geometry
    /// with `maximized`; a harness without a monitor and a minimised window record nothing.
    #[test]
    fn dk6_what_the_window_records() {
        let normal = observed(&info(false), 1.0, None).unwrap();
        assert_eq!(
            normal,
            WindowGeometry {
                x: 100.0,
                y: 50.0,
                width: 1280.0,
                height: 800.0,
                maximized: false
            }
        );
        let max = observed(&info(true), 1.0, Some(normal)).unwrap();
        assert_eq!(
            max,
            WindowGeometry {
                maximized: true,
                ..normal
            }
        );
        assert_eq!(observed(&ViewportInfo::default(), 1.0, Some(normal)), None);
        let minimised = ViewportInfo {
            minimized: Some(true),
            ..info(false)
        };
        assert_eq!(observed(&minimised, 1.0, Some(normal)), None);
        let unscaled = ViewportInfo {
            native_pixels_per_point: None,
            ..info(false)
        };
        assert_eq!(observed(&unscaled, 1.0, Some(normal)), None);
    }

    /// R1-1, R2-1: at the UI scale 2 and the native scale 1.5 egui reports a window of 1280 × 800 logical points
    /// (1920 × 1200 px) with its outer corner at (150, 75) px as 640 × 400 points at (50, 25); the window records the
    /// size in zoom-1 points and the position in pixels.
    #[test]
    fn dk6_the_window_records_logical_size_and_physical_position() {
        let at_zoom_2 = ViewportInfo {
            monitor_size: Some(vec2(960.0, 540.0)),
            outer_rect: Some(Rect::from_min_size(pos2(50.0, 25.0), vec2(648.0, 419.5))),
            inner_rect: Some(Rect::from_min_size(pos2(54.0, 40.5), vec2(640.0, 400.0))),
            maximized: Some(false),
            native_pixels_per_point: Some(1.5),
            ..ViewportInfo::default()
        };
        assert_eq!(
            observed(&at_zoom_2, 2.0, None),
            Some(WindowGeometry {
                x: 150.0,
                y: 75.0,
                width: 1280.0,
                height: 800.0,
                maximized: false
            })
        );
    }

    /// R2-1: a desktop of a 150 % primary (1920 × 1080 px) and a 100 % secondary to its right (2560 × 1440 px at
    /// x = 1920), the UI scale 1.25. A window saved on the secondary at (2200, 100) px opens on the primary from the
    /// builder's guess (2200 points → 3300 px), the first frame moves it back to 2200 px, and it records 2200 px again:
    /// start after start, no drift and no shrink (it used to go 2200 → 3300 → 4950 px, then onto the primary).
    #[test]
    fn dk6_a_round_trip_on_a_mixed_scale_desktop_keeps_the_place_and_the_size() {
        let monitors = desk(&[
            (0.0, 0.0, 1920.0, 1080.0, true),
            (1920.0, 0.0, 2560.0, 1440.0, false),
        ]);
        let zoom = 1.25;
        let mut saved = WindowGeometry {
            x: 2200.0,
            y: 100.0,
            width: 1280.0,
            height: 800.0,
            maximized: false,
        };
        for _ in 0..5 {
            let b = restore(ViewportBuilder::default(), Some(saved));
            let (guess, size) = (b.position.unwrap(), b.inner_size.unwrap());
            // winit creates the window on the primary and converts the builder's points with its scale 1.5.
            let created = 1.5;
            let to = first_frame_position(
                Some((saved.x, saved.y)),
                (guess.x * created, guess.y * created),
                (size.x * created, size.y * created),
                &monitors,
                created,
                1.0,
            )
            .expect("the window moves to its saved place");
            let px = (to.x * created, to.y * created);
            assert_eq!(px, (2200.0, 100.0));
            // On the secondary (100 %) the system keeps the logical size; egui reports it at the zoom 1.25.
            let outer = Rect::from_min_size(pos2(px.0, px.1), size + vec2(16.0, 39.0));
            let inner = Rect::from_min_size(pos2(px.0 + 8.0, px.1 + 31.0), size);
            let reported = ViewportInfo {
                monitor_size: Some(vec2(2560.0, 1440.0) / zoom),
                outer_rect: Some(outer / zoom),
                inner_rect: Some(inner / zoom),
                maximized: Some(false),
                native_pixels_per_point: Some(1.0),
                ..ViewportInfo::default()
            };
            let next = observed(&reported, zoom, Some(saved)).unwrap();
            assert!(
                (next.x - 2200.0).abs() < 1e-3 && (next.y - 100.0).abs() < 1e-3,
                "{next:?}"
            );
            assert!(
                (next.width - 1280.0).abs() < 1e-3 && (next.height - 800.0).abs() < 1e-3,
                "{next:?}"
            );
            saved = next;
        }
    }

    /// R2-1: a saved position on no monitor (a monitor gone, or a value from before R2-1 in points) is clamped on the
    /// first frame; a window already at its saved place does not move; no saved position runs the clamp on where the
    /// window opened.
    #[test]
    fn dk6_the_first_frame_clamps_the_saved_position() {
        let monitors = desk(&[(0.0, 0.0, 1920.0, 1080.0, true)]);
        let to = first_frame_position(
            Some((-2400.0, 40.0)),
            (100.0, 100.0),
            (1280.0, 800.0),
            &monitors,
            1.0,
            1.0,
        );
        assert_eq!(to, Some(pos2(0.0, 40.0)));
        let huge = first_frame_position(
            Some((1e30, 1e30)),
            (100.0, 100.0),
            (1280.0, 800.0),
            &monitors,
            1.0,
            1.0,
        );
        assert_eq!(huge, Some(pos2(640.0, 280.0)));
        let stays = first_frame_position(
            Some((100.0, 50.0)),
            (100.0, 50.0),
            (1280.0, 800.0),
            &monitors,
            1.0,
            1.0,
        );
        assert_eq!(stays, None);
        let none = first_frame_position(None, (100.0, 50.0), (1280.0, 800.0), &monitors, 1.0, 1.0);
        assert_eq!(none, None);
        let nan = first_frame_position(
            Some((f32::NAN, 0.0)),
            (100.0, 50.0),
            (1280.0, 800.0),
            &monitors,
            1.0,
            1.0,
        );
        assert_eq!(nan, None);
    }

    /// R1-4 (FT-526): a window that opened on a monitor that is gone moves onto the primary one; one on any monitor
    /// (the left one at negative x included) stays; without a monitor list nothing moves.
    #[test]
    fn dk6_the_first_frame_moves_an_off_screen_window_onto_the_primary_monitor() {
        let primary = MonitorArea {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let left = MonitorArea {
            x: -2560.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        // Saved on the left monitor, which is unplugged now.
        assert_eq!(
            placement((-2400.0, 40.0), (1280.0, 800.0), &[primary]),
            Some((0.0, 40.0))
        );
        assert_eq!(
            placement((-2400.0, 40.0), (1280.0, 800.0), &[primary, left]),
            None
        );
        assert_eq!(placement((100.0, 50.0), (1280.0, 800.0), &[primary]), None);
        // Far below the desktop.
        assert_eq!(
            placement((100.0, 5000.0), (1280.0, 800.0), &[primary]),
            Some((100.0, 280.0))
        );
        assert_eq!(placement((-2400.0, 40.0), (1280.0, 800.0), &[]), None);
    }

    /// Monitor areas in physical pixels, each with whether it is the primary.
    fn desk(areas: &[(f32, f32, f32, f32, bool)]) -> Vec<(MonitorArea, bool)> {
        areas
            .iter()
            .map(|&(x, y, width, height, primary)| (MonitorArea { x, y, width, height }, primary))
            .collect()
    }

    /// R2-2: the first frame's conversion at the scale 1.5 and the zoom 2. The primary (listed second) is at 150 %:
    /// 2880 × 1620 px at (0, 0); a window that opened at (-3000, 60) px, on a monitor that is gone, moves onto the
    /// primary at x = 0 and keeps its y; egui-winit maps the command back to physical pixels by `zoom × scale`.
    #[test]
    fn dk6_the_first_frame_converts_physical_pixels_at_a_scale_and_a_zoom() {
        let monitors = desk(&[
            (2880.0, 0.0, 2560.0, 1440.0, false),
            (0.0, 0.0, 2880.0, 1620.0, true),
        ]);
        let (scale, zoom) = (1.5, 2.0);
        let size = (1920.0, 1200.0);
        let to = first_frame_position(None, (-3000.0, 60.0), size, &monitors, scale, zoom).unwrap();
        assert_eq!((to.x * zoom * scale, to.y * zoom * scale), (0.0, 60.0));
        // On the secondary monitor: it stays.
        assert_eq!(
            first_frame_position(None, (3000.0, 60.0), size, &monitors, scale, zoom),
            None
        );
        // The primary comes first whatever the system's order: a window off every monitor lands on it, not on the
        // secondary that is listed first.
        let to = first_frame_position(None, (9000.0, 60.0), size, &monitors, scale, zoom).unwrap();
        assert!(to.x * zoom * scale < 2880.0, "{to:?}");
        // No monitor, or a scale or zoom that is not a positive number: nothing moves.
        assert_eq!(
            first_frame_position(None, (-3000.0, 60.0), size, &[], scale, zoom),
            None
        );
        assert_eq!(
            first_frame_position(None, (-3000.0, 60.0), size, &monitors, 0.0, zoom),
            None
        );
        assert_eq!(
            first_frame_position(None, (-3000.0, 60.0), size, &monitors, scale, f32::NAN),
            None
        );
    }

    /// DK6: the builder takes the saved geometry, the size raised to the minimum; without one it is unchanged.
    #[test]
    fn dk6_the_builder_restores_the_saved_geometry() {
        let saved = WindowGeometry {
            x: -300.0,
            y: 20.0,
            width: 500.0,
            height: 900.0,
            maximized: true,
        };
        let b = restore(ViewportBuilder::default(), Some(saved));
        assert_eq!(b.inner_size, Some(vec2(640.0, 900.0)));
        assert_eq!(b.position, Some(pos2(-300.0, 20.0)));
        assert_eq!(b.maximized, Some(true));
        let none = restore(ViewportBuilder::default().with_inner_size([1280.0, 800.0]), None);
        assert_eq!(none.inner_size, Some(vec2(1280.0, 800.0)));
        assert_eq!(none.position, None);
    }
}
