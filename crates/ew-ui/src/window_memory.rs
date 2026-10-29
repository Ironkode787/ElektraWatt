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
//! **Units: points at the zoom 1.** egui reports the window's rectangles in its points at the current zoom (the
//! physical pixels divided by the native scale times egui's zoom factor, which is the UI scale), and eframe opens the
//! window before the UI scale is applied, so the builder's size and position are taken at the zoom 1. The recorded
//! geometry is therefore the reported one times the zoom factor of the frame the report was made with: a window
//! keeps its size and place across starts at every UI scale. The report of a frame is made before the frame applies
//! a new zoom factor (egui applies [`egui::Context::set_zoom_factor`] at the start of the next pass), so a frame whose
//! zoom differs from the previous frame's records nothing.
//!
//! **The clamp at start** (FT-526). eframe 0.37 without its `persistence` feature knows no monitor before the window
//! opens, so the builder gets the saved geometry with the size bounds of [`ew_viewmodel::window::restore`] only (and
//! eframe's own clamp of the size to the largest monitor). On the first frame the executable reads the monitors from
//! eframe's winit window and [`placement`] runs the same pure clamp on the window as it opened: a window whose title
//! bar is on no monitor (its monitor gone) moves onto the primary one.

use egui::{ViewportBuilder, ViewportInfo};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::window::{self, MonitorArea, WindowGeometry};

/// The window builder `builder` with the saved geometry `saved` restored (its size, position and maximised state).
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

/// The geometry a running window reports in `info`, its rectangles in points at the zoom factor `zoom`, given the last
/// one recorded; in points at the zoom 1 (see the module documentation); `None` when there is nothing to record (no
/// real window, minimised, full screen).
pub fn observed(info: &ViewportInfo, zoom: f32, last: Option<WindowGeometry>) -> Option<WindowGeometry> {
    if info.monitor_size.is_none() || info.minimized == Some(true) || info.fullscreen == Some(true) {
        return None;
    }
    let (outer, inner) = (info.outer_rect? * zoom, info.inner_rect? * zoom);
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
    }

    /// R1-1: at the UI scale 2 egui reports a 1280 × 800 window at (100, 50) as 640 × 400 points at (50, 25); the
    /// window records the zoom-1 points, which the builder takes back at the next start.
    #[test]
    fn dk6_the_window_records_points_at_the_zoom_1() {
        let at_zoom_2 = ViewportInfo {
            monitor_size: Some(vec2(960.0, 540.0)),
            outer_rect: Some(Rect::from_min_size(pos2(50.0, 25.0), vec2(648.0, 419.5))),
            inner_rect: Some(Rect::from_min_size(pos2(54.0, 40.5), vec2(640.0, 400.0))),
            maximized: Some(false),
            ..ViewportInfo::default()
        };
        assert_eq!(
            observed(&at_zoom_2, 2.0, None),
            Some(WindowGeometry {
                x: 100.0,
                y: 50.0,
                width: 1280.0,
                height: 800.0,
                maximized: false
            })
        );
    }

    /// R1-1: a window restored at the zoom 1 and reported at the UI scale 1.25 records the geometry it was opened
    /// with, start after start (it used to shrink 1280 → 1024 → 819 → …).
    #[test]
    fn dk6_a_round_trip_at_a_ui_scale_keeps_the_size() {
        let zoom = 1.25;
        let mut saved = WindowGeometry {
            x: -2400.0,
            y: 40.0,
            width: 1280.0,
            height: 800.0,
            maximized: false,
        };
        for _ in 0..5 {
            let b = restore(ViewportBuilder::default(), Some(saved));
            // eframe opens the window at the zoom 1: the builder's points are the desktop's logical points.
            let (pos, size) = (b.position.unwrap(), b.inner_size.unwrap());
            // egui then reports them in points at the zoom 1.25.
            let reported = ViewportInfo {
                monitor_size: Some(vec2(2560.0, 1440.0) / zoom),
                outer_rect: Some(Rect::from_min_size(pos, size + vec2(16.0, 39.0)) / zoom),
                inner_rect: Some(Rect::from_min_size(pos + vec2(8.0, 31.0), size) / zoom),
                maximized: Some(false),
                ..ViewportInfo::default()
            };
            let next = observed(&reported, zoom, Some(saved)).unwrap();
            assert!((next.width - 1280.0).abs() < 1e-3, "{next:?}");
            assert!((next.height - 800.0).abs() < 1e-3, "{next:?}");
            assert!((next.x + 2400.0).abs() < 1e-3, "{next:?}");
            saved = next;
        }
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
