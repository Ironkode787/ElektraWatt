// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Window memory in the window (WP-S2.17 part 3 item 9, DK6; FT-348): the window builder from the saved geometry, and
//! the geometry the running window reports, recorded in the shell's settings (written once, at the end of the window).
//!
//! Only a real window records: egui reports the outer rectangle and the monitor's size only there (a test harness
//! reports neither), so a test's settings file keeps its bytes. A minimised or full-screen window records nothing; a
//! maximised one keeps its last normal size and position with `maximized = true`.
//!
//! **The clamp at start.** [`ew_viewmodel::window::restore`] runs with the monitors the window knows before it opens,
//! which is none: eframe 0.37 without its `persistence` feature exposes no monitor list, so the start applies the size
//! bounds of the clamp and eframe's own clamp of the size to the largest monitor; the position clamp to a visible
//! monitor is the pure function's, tested on three layouts, and waits for a monitor list to be wired.

use egui::{ViewportBuilder, ViewportInfo};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::window::{self, WindowGeometry};

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

/// The geometry a running window reports in `info`, given the last one recorded; `None` when there is nothing to
/// record (no real window, minimised, full screen).
pub fn observed(info: &ViewportInfo, last: Option<WindowGeometry>) -> Option<WindowGeometry> {
    if info.monitor_size.is_none() || info.minimized == Some(true) || info.fullscreen == Some(true) {
        return None;
    }
    let (outer, inner) = (info.outer_rect?, info.inner_rect?);
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

/// Records the geometry of the window of `ctx` in `shell` when it changed.
pub(crate) fn remember(ctx: &egui::Context, shell: &mut ShellModel) {
    let last = shell.settings().window;
    if let Some(g) = ctx.input(|i| observed(i.viewport(), last))
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
        let normal = observed(&info(false), None).unwrap();
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
        let max = observed(&info(true), Some(normal)).unwrap();
        assert_eq!(
            max,
            WindowGeometry {
                maximized: true,
                ..normal
            }
        );
        assert_eq!(observed(&ViewportInfo::default(), Some(normal)), None);
        let minimised = ViewportInfo {
            minimized: Some(true),
            ..info(false)
        };
        assert_eq!(observed(&minimised, Some(normal)), None);
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
