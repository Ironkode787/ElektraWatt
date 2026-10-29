// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one placement function of every canvas chip (WP-S2.17 D13; doc 16a §7.4): preview chips, the ghost's label,
//! highlight name chips and measurement readouts all stand where [`place_chip`] puts them, so collisions are a
//! unit-tested rule instead of each painter's own.
//!
//! **The rule.** Four candidate positions in order, each 8 points off the target: above right (the preferred
//! anchor: the chip's bottom-left corner 8 points right of and above the target's top-right corner), below right,
//! above left, below left. A candidate is rejected when it overlaps an obstacle (another chip, the tool HUD, the
//! frame-statistics panel, the zoom control, the outside of the canvas: the caller passes them, see [`outside`]) or
//! the pointer's 16-point box (centred on the pointer). The first candidate left is taken; when all four fail the
//! chip stands at the preferred anchor, drawn at 80 % opacity (`faded`). Since every candidate keeps 8 points off
//! the target, a chip never covers the target's own printed text.
//!
//! Pure and deterministic: the same input gives the same placement on every platform. Coordinates are logical points
//! of the window (the toolkit's), with their own small types so that this crate stays toolkit-free.

/// A point (logical points).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Pos2 {
    /// Right.
    pub x: f32,
    /// Down.
    pub y: f32,
}

impl Pos2 {
    /// The point `(x, y)`.
    pub const fn new(x: f32, y: f32) -> Pos2 {
        Pos2 { x, y }
    }
}

/// A size (logical points).
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    /// Width.
    pub x: f32,
    /// Height.
    pub y: f32,
}

impl Vec2 {
    /// The size `x × y`.
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
}

/// An axis-aligned rectangle (logical points), `min` top-left and `max` bottom-right.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    /// The top-left corner.
    pub min: Pos2,
    /// The bottom-right corner.
    pub max: Pos2,
}

impl Rect {
    /// The rectangle from its corners.
    pub const fn from_min_max(min: Pos2, max: Pos2) -> Rect {
        Rect { min, max }
    }

    /// The rectangle at `min` of `size`.
    pub fn from_min_size(min: Pos2, size: Vec2) -> Rect {
        Rect {
            min,
            max: Pos2::new(min.x + size.x, min.y + size.y),
        }
    }

    /// The empty rectangle at a point (a chip's target that is a point).
    pub const fn point(p: Pos2) -> Rect {
        Rect { min: p, max: p }
    }

    /// The width.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// The height.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Whether the two rectangles share interior area (touching edges do not overlap).
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }
}

/// Where a chip stands.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChipPlacement {
    /// The chip's frame.
    pub rect: Rect,
    /// No candidate was free: the chip stands at the preferred anchor, drawn at 80 % opacity.
    pub faded: bool,
}

/// The gap between a chip and its target (doc 16a §7.4).
pub const CHIP_OFFSET: f32 = 8.0;

/// The side of the pointer's box chips keep out of (doc 16a §7.4, FT-508).
pub const POINTER_BOX: f32 = 16.0;

/// The opacity of a faded chip.
pub const FADED_OPACITY: f32 = 0.8;

/// The four candidates for a chip of `size` by `target`, in the rule's order: above right, below right, above left,
/// below left.
pub fn candidates(target: Rect, size: Vec2) -> [Rect; 4] {
    let o = CHIP_OFFSET;
    let right = target.max.x + o;
    let left = target.min.x - o - size.x;
    let above = target.min.y - o - size.y;
    let below = target.max.y + o;
    [
        Rect::from_min_size(Pos2::new(right, above), size),
        Rect::from_min_size(Pos2::new(right, below), size),
        Rect::from_min_size(Pos2::new(left, above), size),
        Rect::from_min_size(Pos2::new(left, below), size),
    ]
}

/// The pointer's box: [`POINTER_BOX`] points square, centred on the pointer.
pub fn pointer_box(p: Pos2) -> Rect {
    let h = POINTER_BOX / 2.0;
    Rect::from_min_max(Pos2::new(p.x - h, p.y - h), Pos2::new(p.x + h, p.y + h))
}

/// Places a chip of `size` by `target` (doc 16a §7.4): the first of [`candidates`] that overlaps no obstacle and not
/// the pointer's box, else the preferred anchor, faded.
pub fn place_chip(target: Rect, size: Vec2, obstacles: &[Rect], pointer: Option<Pos2>) -> ChipPlacement {
    let all = candidates(target, size);
    let pointer = pointer.map(pointer_box);
    let free = |r: &Rect| !obstacles.iter().chain(pointer.as_ref()).any(|o| o.overlaps(r));
    match all.iter().find(|r| free(r)) {
        Some(rect) => ChipPlacement {
            rect: *rect,
            faded: false,
        },
        None => ChipPlacement {
            rect: all[0],
            faded: true,
        },
    }
}

/// The outside of `bounds` as four obstacles (above, below, left, right), so that [`place_chip`] keeps a chip inside
/// the canvas. Each reaches far enough (10⁶ points) to hold any chip.
pub fn outside(bounds: Rect) -> [Rect; 4] {
    const FAR: f32 = 1.0e6;
    let (a, b) = (bounds.min, bounds.max);
    [
        Rect::from_min_max(Pos2::new(a.x - FAR, a.y - FAR), Pos2::new(b.x + FAR, a.y)),
        Rect::from_min_max(Pos2::new(a.x - FAR, b.y), Pos2::new(b.x + FAR, b.y + FAR)),
        Rect::from_min_max(Pos2::new(a.x - FAR, a.y), Pos2::new(a.x, b.y)),
        Rect::from_min_max(Pos2::new(b.x, a.y), Pos2::new(b.x + FAR, b.y)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Vec2 = Vec2::new(60.0, 20.0);

    fn target() -> Rect {
        Rect::point(Pos2::new(200.0, 200.0))
    }

    fn canvas() -> Vec<Rect> {
        outside(Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(800.0, 600.0))).to_vec()
    }

    #[test]
    fn ov2_the_preferred_anchor_is_above_right_8_points_off() {
        let p = place_chip(target(), SIZE, &canvas(), None);
        assert!(!p.faded);
        assert_eq!(p.rect.min, Pos2::new(208.0, 172.0));
        assert_eq!(p.rect.max, Pos2::new(268.0, 192.0));
        // A rectangular target: off its top-right corner.
        let r = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(150.0, 130.0));
        let p = place_chip(r, SIZE, &[], None);
        assert_eq!(p.rect.min, Pos2::new(158.0, 72.0));
        assert!(!p.rect.overlaps(&r), "never over the target");
    }

    #[test]
    fn ov2_each_fallback_in_order() {
        let c = candidates(target(), SIZE);
        // Above right taken: below right.
        let p = place_chip(target(), SIZE, &[c[0]], None);
        assert_eq!((p.rect, p.faded), (c[1], false));
        assert_eq!(p.rect.min, Pos2::new(208.0, 208.0));
        // Both right taken: above left.
        let p = place_chip(target(), SIZE, &[c[0], c[1]], None);
        assert_eq!((p.rect, p.faded), (c[2], false));
        assert_eq!(p.rect.min, Pos2::new(132.0, 172.0));
        // Three taken: below left.
        let p = place_chip(target(), SIZE, &[c[0], c[1], c[2]], None);
        assert_eq!((p.rect, p.faded), (c[3], false));
        assert_eq!(p.rect.min, Pos2::new(132.0, 208.0));
    }

    #[test]
    fn ov2_all_rejected_is_the_preferred_anchor_faded() {
        let c = candidates(target(), SIZE);
        let p = place_chip(target(), SIZE, &c, None);
        assert!(p.faded);
        assert_eq!(p.rect, c[0]);
        assert_eq!(FADED_OPACITY, 0.8);
    }

    /// One obstacle case: the target, the obstacles besides the canvas outside, the pointer, the expected
    /// candidate (by index) and a rectangle the chip must or must not cover.
    struct ObstacleCase {
        name: &'static str,
        target: Rect,
        extra: Vec<Rect>,
        pointer: Option<Pos2>,
        want: Option<usize>,
        avoids: Option<Rect>,
        covers: Option<Rect>,
    }

    /// The frame-statistics panel (FT-508), the tool HUD and the pointer's 16-point box are obstacles like any
    /// chip.
    #[test]
    fn ov2_the_hud_is_an_obstacle() {
        // The statistics panel at the canvas's top right.
        let panel = Rect::from_min_max(Pos2::new(600.0, 12.0), Pos2::new(788.0, 140.0));
        let under_panel = Rect::point(Pos2::new(650.0, 135.0));
        // The tool HUD at the canvas's bottom centre.
        let hud = Rect::from_min_max(Pos2::new(250.0, 548.0), Pos2::new(550.0, 584.0));
        let over_hud = Rect::point(Pos2::new(300.0, 530.0));
        let cases = [
            ObstacleCase {
                name: "without the panel the chip would stand under it",
                target: under_panel,
                extra: vec![],
                pointer: None,
                want: None,
                avoids: None,
                covers: Some(panel),
            },
            ObstacleCase {
                name: "the statistics panel: below right, under the panel",
                target: under_panel,
                extra: vec![panel],
                pointer: None,
                want: Some(1),
                avoids: Some(panel),
                covers: None,
            },
            ObstacleCase {
                name: "the HUD: above right stays free",
                target: over_hud,
                extra: vec![hud],
                pointer: None,
                want: Some(0),
                avoids: Some(hud),
                covers: None,
            },
            // Below the target only: the HUD takes both lower candidates, the upper ones are free.
            ObstacleCase {
                name: "the HUD with above right taken: above left",
                target: over_hud,
                extra: vec![candidates(over_hud, SIZE)[0], hud],
                pointer: None,
                want: Some(2),
                avoids: None,
                covers: None,
            },
            ObstacleCase {
                name: "the pointer inside the preferred candidate",
                target: target(),
                extra: vec![],
                pointer: Some(Pos2::new(230.0, 185.0)),
                want: Some(1),
                avoids: None,
                covers: None,
            },
            ObstacleCase {
                name: "the pointer's box reaches in from 7 points outside the left edge",
                target: target(),
                extra: vec![],
                pointer: Some(Pos2::new(201.0, 180.0)),
                want: Some(1),
                avoids: None,
                covers: None,
            },
            ObstacleCase {
                name: "8 points outside the box only touches the edge, the candidate stays",
                target: target(),
                extra: vec![],
                pointer: Some(Pos2::new(200.0, 180.0)),
                want: Some(0),
                avoids: None,
                covers: None,
            },
        ];
        for c in cases {
            let mut obstacles = canvas();
            obstacles.extend(c.extra);
            let p = place_chip(c.target, SIZE, &obstacles, c.pointer);
            assert!(!p.faded, "case {}: {p:?}", c.name);
            if let Some(i) = c.want {
                assert_eq!(p.rect, candidates(c.target, SIZE)[i], "case {}", c.name);
            }
            if let Some(r) = c.avoids {
                assert!(!p.rect.overlaps(&r), "case {}: {p:?}", c.name);
            }
            if let Some(r) = c.covers {
                assert!(p.rect.overlaps(&r), "case {}: {p:?}", c.name);
            }
        }
        assert_eq!(pointer_box(Pos2::new(10.0, 10.0)).width(), 16.0);
    }

    /// The outside of the canvas: near the top-left corner the preferred anchor would leave it.
    #[test]
    fn ov2_chips_stay_inside_the_canvas() {
        let t = Rect::point(Pos2::new(4.0, 4.0));
        let p = place_chip(t, SIZE, &canvas(), None);
        assert_eq!(p.rect.min, Pos2::new(12.0, 12.0), "below right");
        let t = Rect::point(Pos2::new(796.0, 596.0));
        let p = place_chip(t, SIZE, &canvas(), None);
        assert_eq!(p.rect, candidates(t, SIZE)[2], "above left");
    }

    /// Chips placed one after another, each an obstacle for the next, never intersect.
    #[test]
    fn ov2_successive_chips_never_intersect() {
        let mut obstacles = canvas();
        let mut placed = Vec::new();
        for k in 0..4 {
            let t = Rect::point(Pos2::new(300.0 + k as f32, 300.0));
            let p = place_chip(t, SIZE, &obstacles, None);
            assert!(!p.faded, "chip {k}");
            obstacles.push(p.rect);
            placed.push(p.rect);
        }
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                assert!(!a.overlaps(b), "{a:?} {b:?}");
            }
        }
        // A fifth at the same spot finds no free candidate: faded at the preferred anchor.
        let p = place_chip(Rect::point(Pos2::new(300.0, 300.0)), SIZE, &obstacles, None);
        assert!(p.faded);
    }

    #[test]
    fn touching_edges_do_not_overlap() {
        let a = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0));
        let b = Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(20.0, 10.0));
        assert!(!a.overlaps(&b));
        assert!(a.overlaps(&Rect::from_min_max(Pos2::new(9.0, 9.0), Pos2::new(11.0, 11.0))));
        assert_eq!(a.height(), 10.0);
    }
}
