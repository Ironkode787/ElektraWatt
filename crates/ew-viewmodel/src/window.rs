// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Window memory (WP-S2.17 part 3 item 9, check DK6; the rest of FT-348): the main window's size, position and
//! maximised state, kept in `settings.toml` as `[window]` and restored at the next start, clamped so that the window
//! opens on a monitor the system still has.
//!
//! **Units.** Logical points of the desktop (egui's points at the UI scale 1), whole numbers in the file. The position
//! is the outer top-left corner (title bar included), the size the inner size: the two values a window builder takes.
//!
//! **The clamp** ([`restore`]) is a pure function of the saved geometry and the monitors' areas: the size is at least
//! [`MIN_SIZE`] and at most the largest monitor; the saved position stays when a grip of the title bar ([`GRIP`] wide
//! and tall) lies on a monitor, and the size is then at most that monitor's; otherwise the window moves into the first
//! monitor (the primary), as close to its saved place as fits. The maximised state is kept.

/// The smallest window, in points (the window builder's minimum).
pub const MIN_SIZE: (f32, f32) = (640.0, 400.0);

/// The part of the title bar that must lie on a monitor for the saved position to stay: so many points from the
/// window's top-left corner, across and down (enough to grab and move it).
pub const GRIP: f32 = 48.0;

/// The window's geometry, in points.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WindowGeometry {
    /// The outer left edge.
    pub x: f32,
    /// The outer top edge.
    pub y: f32,
    /// The inner width.
    pub width: f32,
    /// The inner height.
    pub height: f32,
    /// Whether the window was maximised (the other values are then its size and position before it was).
    pub maximized: bool,
}

/// A monitor's area on the desktop, in points.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MonitorArea {
    /// The left edge.
    pub x: f32,
    /// The top edge.
    pub y: f32,
    /// The width.
    pub width: f32,
    /// The height.
    pub height: f32,
}

impl MonitorArea {
    /// The overlap of this area with the rectangle `(x, y, width, height)`: its width and height (0 when apart).
    fn overlap(&self, x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
        let w = (self.x + self.width).min(x + width) - self.x.max(x);
        let h = (self.y + self.height).min(y + height) - self.y.max(y);
        (w.max(0.0), h.max(0.0))
    }
}

impl WindowGeometry {
    /// The geometry of a `[window]` table: `x`, `y`, `width`, `height` (numbers) and `maximized` (a boolean, absent is
    /// `false`); `None` when a member is missing, of another type or not finite.
    pub fn from_toml(table: &toml::Table) -> Option<WindowGeometry> {
        let number = |key: &str| -> Option<f32> {
            let v = match table.get(key)? {
                toml::Value::Integer(i) => *i as f32,
                toml::Value::Float(f) => *f as f32,
                _ => return None,
            };
            v.is_finite().then_some(v)
        };
        let maximized = match table.get("maximized") {
            None => false,
            Some(toml::Value::Boolean(b)) => *b,
            Some(_) => return None,
        };
        Some(WindowGeometry {
            x: number("x")?,
            y: number("y")?,
            width: number("width")?,
            height: number("height")?,
            maximized,
        })
    }

    /// The `[window]` table: the four numbers rounded to whole points, then `maximized`.
    pub fn to_toml(&self) -> toml::Table {
        let mut t = toml::Table::new();
        for (key, v) in [
            ("x", self.x),
            ("y", self.y),
            ("width", self.width),
            ("height", self.height),
        ] {
            t.insert(key.to_owned(), toml::Value::Integer(v.round() as i64));
        }
        t.insert("maximized".to_owned(), toml::Value::Boolean(self.maximized));
        t
    }
}

/// The geometry to open the window with, from the `saved` one and the system's `monitors` (the primary first): see the
/// module documentation. With no monitor known, only the size's lower bound applies.
pub fn restore(saved: WindowGeometry, monitors: &[MonitorArea]) -> WindowGeometry {
    let mut g = saved;
    g.width = g.width.max(MIN_SIZE.0);
    g.height = g.height.max(MIN_SIZE.1);
    let Some(primary) = monitors.first() else {
        return g;
    };
    let largest = monitors
        .iter()
        .fold((0.0f32, 0.0f32), |(w, h), m| (w.max(m.width), h.max(m.height)));
    g.width = g.width.min(largest.0.max(MIN_SIZE.0));
    g.height = g.height.min(largest.1.max(MIN_SIZE.1));
    let grip_w = GRIP.min(g.width);
    let on = monitors.iter().find(|m| {
        let (w, h) = m.overlap(g.x, g.y, g.width, GRIP);
        w >= grip_w && h >= GRIP
    });
    let target = match on {
        Some(m) => {
            g.width = g.width.min(m.width.max(MIN_SIZE.0));
            g.height = g.height.min(m.height.max(MIN_SIZE.1));
            return g;
        }
        None => primary,
    };
    g.width = g.width.min(target.width.max(MIN_SIZE.0));
    g.height = g.height.min(target.height.max(MIN_SIZE.1));
    g.x =
        g.x.clamp(target.x, (target.x + target.width - g.width).max(target.x));
    g.y =
        g.y.clamp(target.y, (target.y + target.height - g.height).max(target.y));
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(x: f32, y: f32, width: f32, height: f32) -> WindowGeometry {
        WindowGeometry {
            x,
            y,
            width,
            height,
            maximized: false,
        }
    }

    fn area(x: f32, y: f32, width: f32, height: f32) -> MonitorArea {
        MonitorArea { x, y, width, height }
    }

    /// DK6, layout 1: one 1920 × 1080 monitor. A window on it stays; one beyond its right edge moves back onto it; a
    /// window larger than the monitor shrinks to it; a tiny one grows to the minimum.
    #[test]
    fn dk6_one_monitor() {
        let m = [area(0.0, 0.0, 1920.0, 1080.0)];
        assert_eq!(
            restore(geo(100.0, 80.0, 1280.0, 800.0), &m),
            geo(100.0, 80.0, 1280.0, 800.0)
        );
        assert_eq!(
            restore(geo(2500.0, 80.0, 1280.0, 800.0), &m),
            geo(640.0, 80.0, 1280.0, 800.0)
        );
        assert_eq!(
            restore(geo(-50.0, -900.0, 1280.0, 800.0), &m),
            geo(0.0, 0.0, 1280.0, 800.0)
        );
        assert_eq!(
            restore(geo(0.0, 0.0, 3000.0, 2000.0), &m),
            geo(0.0, 0.0, 1920.0, 1080.0)
        );
        assert_eq!(
            restore(geo(10.0, 10.0, 100.0, 50.0), &m),
            geo(10.0, 10.0, 640.0, 400.0)
        );
        // Half off the right edge, the title bar's grip still on the monitor: it stays where the user left it.
        assert_eq!(
            restore(geo(1500.0, 200.0, 1280.0, 800.0), &m),
            geo(1500.0, 200.0, 1280.0, 800.0)
        );
    }

    /// DK6, layout 2: a primary 1920 × 1080 and a 2560 × 1440 monitor to its left (negative coordinates). A window on
    /// the left monitor stays there with its larger size; the maximised state is kept.
    #[test]
    fn dk6_two_monitors_one_at_negative_coordinates() {
        let m = [
            area(0.0, 0.0, 1920.0, 1080.0),
            area(-2560.0, -200.0, 2560.0, 1440.0),
        ];
        let left = WindowGeometry {
            maximized: true,
            ..geo(-2400.0, -100.0, 2200.0, 1300.0)
        };
        assert_eq!(restore(left, &m), left);
        // Above both monitors: back onto the primary, as close as fits.
        assert_eq!(
            restore(geo(300.0, -1500.0, 1280.0, 800.0), &m),
            geo(300.0, 0.0, 1280.0, 800.0)
        );
    }

    /// DK6, layout 3: the secondary monitor the window was on is gone (a laptop off its dock): the window moves into the
    /// primary 1366 × 768 and shrinks to it.
    #[test]
    fn dk6_the_monitor_it_was_on_is_gone() {
        let m = [area(0.0, 0.0, 1366.0, 768.0)];
        let saved = WindowGeometry {
            maximized: true,
            ..geo(1920.0, 100.0, 1600.0, 1000.0)
        };
        let expected = WindowGeometry {
            maximized: true,
            ..geo(0.0, 0.0, 1366.0, 768.0)
        };
        assert_eq!(restore(saved, &m), expected);
        // No monitor known: the geometry as saved, the size at least the minimum.
        assert_eq!(
            restore(geo(5000.0, 9.0, 10.0, 10.0), &[]),
            geo(5000.0, 9.0, 640.0, 400.0)
        );
    }

    /// The clamp is deterministic and idempotent over a grid of positions on every layout.
    #[test]
    fn dk6_restore_is_idempotent() {
        let layouts: [&[MonitorArea]; 3] = [
            &[area(0.0, 0.0, 1920.0, 1080.0)],
            &[
                area(0.0, 0.0, 1920.0, 1080.0),
                area(-2560.0, -200.0, 2560.0, 1440.0),
            ],
            &[area(0.0, 0.0, 1366.0, 768.0), area(1366.0, 0.0, 1920.0, 1080.0)],
        ];
        for m in layouts {
            for x in (-4000..4000).step_by(450) {
                for y in (-2000..2000).step_by(370) {
                    let once = restore(geo(x as f32, y as f32, 1500.0, 900.0), m);
                    assert_eq!(restore(once, m), once, "{x} {y}");
                    let grip_on = m.iter().any(|a| {
                        let (w, h) = a.overlap(once.x, once.y, once.width, GRIP);
                        w >= GRIP.min(once.width) && h >= GRIP
                    });
                    assert!(grip_on, "{x} {y}: {once:?}");
                }
            }
        }
    }

    /// The `[window]` table round-trips in whole points; a bad member drops the table.
    #[test]
    fn the_window_table_round_trips() {
        let g = WindowGeometry {
            maximized: true,
            ..geo(-12.4, 30.6, 1280.0, 800.0)
        };
        let back = WindowGeometry::from_toml(&g.to_toml()).unwrap();
        assert_eq!(
            back,
            WindowGeometry {
                x: -12.0,
                y: 31.0,
                ..g
            }
        );
        let mut bad = g.to_toml();
        bad.insert("width".to_owned(), toml::Value::String("wide".to_owned()));
        assert_eq!(WindowGeometry::from_toml(&bad), None);
        bad.remove("width");
        assert_eq!(WindowGeometry::from_toml(&bad), None);
    }
}
