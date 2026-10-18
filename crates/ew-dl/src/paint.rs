// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Colours, brushes and strokes: concrete, hashable paint for the IR.

use ew_units::Nm;

/// An sRGB colour with straight (not premultiplied) alpha, 8 bits per channel.
///
/// The IR keeps its own colour type rather than `peniko::Color` (whose channels are `f32` and therefore
/// neither `Eq` nor `Hash`), so display lists compare and hash by value (ADR-0002 early cut-off).
/// Renderer crates convert. "By layer" colours of doc 07 are resolved before the IR: every colour here is
/// concrete.
///
/// ```
/// use ew_dl::Color;
///
/// assert_eq!(Color::from_hex("#c0c0c0"), Some(Color::from_rgb8(192, 192, 192)));
/// assert_eq!(Color::from_hex("#C0C0C0"), Color::from_hex("#c0c0c0"));
/// assert_eq!(Color::from_hex("c0c0c0"), None);
/// assert_eq!(Color::BLACK.to_hex(), "#000000");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha; 255 is opaque.
    pub a: u8,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Color = Color::from_rgb8(0, 0, 0);
    /// Opaque white.
    pub const WHITE: Color = Color::from_rgb8(255, 255, 255);

    /// An opaque colour.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    /// A colour with alpha.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }

    /// Parses `#rrggbb` (hex digits in either case) as an opaque colour; `None` for anything else.
    pub fn from_hex(s: &str) -> Option<Color> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Color::from_rgb8(channel(0)?, channel(2)?, channel(4)?))
    }

    /// `#rrggbb` in lower case; alpha is not included.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// How a path is filled or a glyph run is painted. Only solid colours so far; gradients and patterns are
/// not needed by any output of the golden project.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Brush {
    /// One colour.
    Solid(Color),
}

impl Brush {
    /// Solid black, the default for drawing pens.
    pub const BLACK: Brush = Brush::Solid(Color::BLACK);
}

/// The shape at the open ends of a stroked path and of each dash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cap {
    /// Ends exactly at the end point (PDF cap 0).
    Butt,
    /// A half disc around the end point (PDF cap 1).
    Round,
    /// A half square beyond the end point (PDF cap 2).
    Square,
}

/// The shape where two segments of a stroked path meet.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Join {
    /// Sharp corner, limited by [`MITER_LIMIT`] (PDF join 0).
    Miter,
    /// Rounded corner (PDF join 1).
    Round,
    /// Cut-off corner (PDF join 2).
    Bevel,
}

/// The miter limit every writer sets explicitly for [`Join::Miter`] (PDF `M`, SVG `stroke-miterlimit`,
/// `kurbo::Stroke::miter_limit`). PDF's default is 10 and SVG's is 4; leaving either default in place would
/// make screen and paper differ at sharp corners (FR-OUT-018), so the value is fixed here: 4, the SVG and
/// kurbo default (corners sharper than about 29° are bevelled).
pub const MITER_LIMIT: f64 = 4.0;

/// A dash pattern in absolute lengths: alternating on and off lengths, starting with "on", and the offset
/// into the pattern at which the path starts.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Dash {
    /// On, off, on, off … lengths. An odd count repeats the list (PDF and SVG semantics).
    pub segments: Vec<Nm>,
    /// Distance into the pattern at the start of each subpath.
    pub offset: Nm,
}

/// The dash patterns of doc 07 §2.3 (FR-SYM-003), given as multiples of the pen width.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DashPattern {
    /// Dashed 4:2: dash 4 × width, gap 2 × width. At 0.26 mm: dash 1.04 mm, gap 0.52 mm (the S51 linkage).
    Dash42,
    /// Dotted 1:2: dot 1 × width, gap 2 × width. With butt caps the dot is a square. At 0.26 mm: dot
    /// 0.26 mm, gap 0.52 mm (the "External Connection" box).
    Dot12,
    /// Dash-dot 4:2:1:2: dash 4 × width, gap 2, dot 1, gap 2 (the ratio given in doc 07 §2.3).
    DashDot,
}

impl DashPattern {
    /// The on/off multiples of the pen width.
    pub const fn multiples(self) -> &'static [i64] {
        match self {
            DashPattern::Dash42 => &[4, 2],
            DashPattern::Dot12 => &[1, 2],
            DashPattern::DashDot => &[4, 2, 1, 2],
        }
    }

    /// The pattern scaled to a pen `width`, starting at offset 0.
    ///
    /// ```
    /// use ew_dl::{DashPattern, Pen};
    /// use ew_units::Nm;
    ///
    /// let d = DashPattern::Dash42.scaled(Pen::P026.width());
    /// assert_eq!(d.segments, [Nm(1_040_000), Nm(520_000)]);
    /// ```
    pub fn scaled(self, width: Nm) -> Dash {
        Dash {
            segments: self.multiples().iter().map(|&m| Nm(width.0 * m)).collect(),
            offset: Nm::ZERO,
        }
    }
}

/// The house pens of doc 07 §2.3 (FR-SYM-003). Explicit widths are plain [`Stroke`] values.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Pen {
    /// 0.13 mm: thin symbol lines.
    P013,
    /// 0.18 mm: report table grid.
    P018,
    /// 0.26 mm: conductors, outlines, frame, grid.
    P026,
    /// 0.42 mm: cover boxes and tables.
    P042,
    /// 0.52 mm: cabinet outline, heavy symbol bars.
    P052,
    /// 1.30 mm: heavy cover and section rules.
    P130,
}

impl Pen {
    /// All pens, thinnest first.
    pub const ALL: [Pen; 6] = [Pen::P013, Pen::P018, Pen::P026, Pen::P042, Pen::P052, Pen::P130];

    /// The pen width.
    pub const fn width(self) -> Nm {
        Nm(match self {
            Pen::P013 => 130_000,
            Pen::P018 => 180_000,
            Pen::P026 => 260_000,
            Pen::P042 => 420_000,
            Pen::P052 => 520_000,
            Pen::P130 => 1_300_000,
        })
    }
}

/// How a path is stroked. Always concrete: pens, dash patterns and colours are resolved before the IR.
///
/// Stroke widths are in nm on the sheet; zoom-independent hairlines do not exist in the IR.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Stroke {
    /// Line width.
    pub width: Nm,
    /// End caps (also of each dash).
    pub cap: Cap,
    /// Corner joins.
    pub join: Join,
    /// Dash pattern; `None` is solid.
    pub dash: Option<Dash>,
}

impl Stroke {
    /// A solid stroke of `width` with butt caps and miter joins.
    pub const fn solid(width: Nm) -> Stroke {
        Stroke {
            width,
            cap: Cap::Butt,
            join: Join::Miter,
            dash: None,
        }
    }

    /// A solid house pen: [`Pen::width`], butt caps (so dashes print at their nominal lengths, FR-SYM-003),
    /// miter joins (crisp frame and box corners, limited by [`MITER_LIMIT`]).
    ///
    /// ```
    /// use ew_dl::{Cap, DashPattern, Pen, Stroke};
    /// use ew_units::Nm;
    ///
    /// let s = Stroke::pen(Pen::P026).dashed(DashPattern::Dot12);
    /// assert_eq!(s.width, Nm(260_000));
    /// assert_eq!(s.cap, Cap::Butt);
    /// assert_eq!(s.dash.unwrap().segments, [Nm(260_000), Nm(520_000)]);
    /// ```
    pub const fn pen(pen: Pen) -> Stroke {
        Stroke::solid(pen.width())
    }

    /// This stroke with `pattern` scaled to its width.
    pub fn dashed(self, pattern: DashPattern) -> Stroke {
        Stroke {
            dash: Some(pattern.scaled(self.width)),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(
            Color::BLACK,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255
            }
        );
        assert_eq!(Color::WHITE.to_hex(), "#ffffff");
        assert_eq!(Color::from_hex("#0aF09b"), Some(Color::from_rgb8(10, 240, 155)));
        for bad in [
            "", "#", "#12345", "#1234567", "#12345g", "123456", "#+12345", "# 12345",
        ] {
            assert_eq!(Color::from_hex(bad), None, "{bad}");
        }
        assert_eq!(Color::from_rgba8(1, 2, 3, 4).a, 4);
        assert_eq!(Color::from_rgba8(1, 2, 3, 4).to_hex(), "#010203");
    }

    #[test]
    fn house_pens() {
        let mm: Vec<f64> = Pen::ALL.iter().map(|p| p.width().to_mm_f64()).collect();
        assert_eq!(mm, [0.13, 0.18, 0.26, 0.42, 0.52, 1.30]);
        let s = Stroke::pen(Pen::P013);
        assert_eq!((s.cap, s.join, s.dash), (Cap::Butt, Join::Miter, None));
    }

    /// FR-SYM-003 acceptance values: dash 1.04 / gap 0.52 mm and dot 0.26 / gap 0.52 mm at 0.26 mm.
    #[test]
    fn dash_patterns_scale_with_the_pen() {
        let w = Pen::P026.width();
        assert_eq!(
            DashPattern::Dash42.scaled(w).segments,
            [Nm::from_um(1_040), Nm::from_um(520)]
        );
        assert_eq!(
            DashPattern::Dot12.scaled(w).segments,
            [Nm::from_um(260), Nm::from_um(520)]
        );
        assert_eq!(
            DashPattern::DashDot.scaled(w).segments,
            [
                Nm::from_um(1_040),
                Nm::from_um(520),
                Nm::from_um(260),
                Nm::from_um(520)
            ]
        );
        let thin = Stroke::pen(Pen::P013).dashed(DashPattern::Dash42);
        assert_eq!(
            thin.dash,
            Some(Dash {
                segments: vec![Nm(520_000), Nm(260_000)],
                offset: Nm(0)
            })
        );
    }
}
