// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The wire-colour view (WP-S2.08 D18, FR-UI-047, doc 16 §10.3): view state of the window, never a model edit and
//! never in a display list, so the PDF and the PNG exports never see it.
//!
//! While View ▸ Wire colours is on, every connection drawn on the shown page whose effective colour is known is
//! stroked [`STROKE_PT`] points wide in that colour over the sheet, with its code at the midpoint of its drawn path
//! (by length). The code is the effective colour's text as the wire list prints it (IEC 60757's two letters, two codes
//! concatenated for a two-colour wire, a company code as the colour profile declares it). Its display colour:
//!
//! - one IEC 60757 code: [`iec_rgb`];
//! - two IEC 60757 codes (`GNYE`): alternating [`DASH_NM`] (2 mm on the sheet) dashes of the first and the second;
//! - a company code that is `D` or `L` before an IEC 60757 code (`DBU`, dark blue): that colour darkened or
//!   lightened ([`company_rgb`]);
//! - any other code: [`UNKNOWN_RGB`], still labelled with its code.
//!
//! A WH or YE wire is cased: a dark outline [`CASING_PX`] physical pixel wide on both sides of its stroke, so it stays
//! visible on the white sheet.

/// The stroke's width in logical points.
pub const STROKE_PT: f64 = 3.0;

/// The casing's width in physical pixels, on each side of a cased stroke.
pub const CASING_PX: f64 = 1.0;

/// The casing's colour (`slate-900`).
pub const CASING_RGB: (u8, u8, u8) = (0x1A, 0x20, 0x2C);

/// A two-colour wire's dash length on the sheet: 2 mm in nanometres.
pub const DASH_NM: f64 = 2_000_000.0;

/// The colour of a code the view does not know (`slate-500`).
pub const UNKNOWN_RGB: (u8, u8, u8) = (0x64, 0x74, 0x8B);

/// The codes whose wires are cased.
pub const CASED: [&str; 2] = ["WH", "YE"];

/// The display colour of an IEC 60757 code.
pub fn iec_rgb(code: &str) -> Option<(u8, u8, u8)> {
    Some(match code {
        "BK" => (0x00, 0x00, 0x00),
        "BN" => (0x8B, 0x4A, 0x1C),
        "RD" => (0xD7, 0x19, 0x1C),
        "OG" => (0xF5, 0x82, 0x1F),
        "YE" => (0xFF, 0xD7, 0x00),
        "GN" => (0x1A, 0x9A, 0x3B),
        "BU" => (0x1F, 0x5F, 0xD6),
        "VT" => (0x7B, 0x2C, 0xBF),
        "GY" => (0x80, 0x80, 0x80),
        "WH" => (0xFF, 0xFF, 0xFF),
        "PK" => (0xF2, 0x8C, 0xB8),
        "GD" => (0xC9, 0xA2, 0x27),
        "TQ" => (0x2B, 0xB5, 0xB0),
        "SR" => (0xB8, 0xB8, 0xB8),
        _ => return None,
    })
}

/// The display colour of a company code `D<IEC>` (darker: each channel × 0.55) or `L<IEC>` (lighter: halfway to
/// white); `None` for any other code.
pub fn company_rgb(code: &str) -> Option<(u8, u8, u8)> {
    let (shade, rest) = code.split_at_checked(1)?;
    let (r, g, b) = iec_rgb(rest)?;
    let dark = |c: u8| (f64::from(c) * 0.55).round() as u8;
    let light = |c: u8| c + (255 - c) / 2;
    match shade {
        "D" => Some((dark(r), dark(g), dark(b))),
        "L" => Some((light(r), light(g), light(b))),
        _ => None,
    }
}

/// The display colours of an effective colour's code: one colour, or two for a two-colour wire (dashed).
pub fn colours_of(code: &str) -> Vec<(u8, u8, u8)> {
    if let Some(c) = iec_rgb(code).or_else(|| company_rgb(code)) {
        return vec![c];
    }
    if let Some((a, b)) = code.split_at_checked(2)
        && let (Some(a), Some(b)) = (iec_rgb(a), iec_rgb(b))
    {
        return vec![a, b];
    }
    vec![UNKNOWN_RGB]
}

/// A piece of a stroke in sheet nanometres: its two ends and the index of its colour in [`WireStroke::colours`].
pub type Piece = ((f64, f64), (f64, f64), usize);

/// One connection of the view on the shown page.
#[derive(Clone, PartialEq, Debug)]
pub struct WireStroke {
    /// The connection.
    pub id: ew_api::output::Uuid,
    /// Its drawn path in sheet nanometres.
    pub polyline: Vec<(i64, i64)>,
    /// Its effective colour's code, as the label shows it.
    pub code: String,
    /// Its display colours ([`colours_of`]): one, or two drawn as alternating dashes.
    pub colours: Vec<(u8, u8, u8)>,
    /// Whether it is cased ([`CASED`]).
    pub cased: bool,
}

impl WireStroke {
    /// The stroke of `code` along `polyline`.
    pub fn new(id: ew_api::output::Uuid, polyline: Vec<(i64, i64)>, code: &str) -> WireStroke {
        WireStroke {
            id,
            polyline,
            code: code.to_owned(),
            colours: colours_of(code),
            cased: CASED.contains(&code),
        }
    }

    /// The pieces to stroke, in sheet nanometres, each with the index of its colour in [`WireStroke::colours`]: the
    /// segments for one colour, the alternating [`DASH_NM`] dashes for two (the first colour first, continuing over the
    /// path's corners).
    pub fn pieces(&self) -> Vec<Piece> {
        let pts: Vec<(f64, f64)> = self.polyline.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let mut out = Vec::new();
        if self.colours.len() < 2 {
            for w in pts.windows(2) {
                if w[0] != w[1] {
                    out.push((w[0], w[1], 0));
                }
            }
            return out;
        }
        // The distance walked along the path so far, and so the dash of each point.
        let mut walked = 0.0f64;
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if !(len.is_finite() && len > 0.0) {
                continue;
            }
            let at = |d: f64| (a.0 + (b.0 - a.0) * d / len, a.1 + (b.1 - a.1) * d / len);
            let mut d = 0.0;
            while d < len {
                let dash = (walked / DASH_NM).floor();
                let end = ((dash + 1.0) * DASH_NM - walked).min(len - d);
                // Rounding never stalls the walk: a dash that ends where it starts takes the next one's length.
                let end = if end > 0.0 { end } else { DASH_NM.min(len - d) };
                out.push((at(d), at(d + end), (dash as usize) % 2));
                d += end;
                walked += end;
            }
        }
        out
    }

    /// The midpoint of the drawn path by length, in sheet nanometres (where the code stands); `None` for an empty path.
    pub fn midpoint(&self) -> Option<(f64, f64)> {
        let pts: Vec<(f64, f64)> = self.polyline.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let first = *pts.first()?;
        let lens: Vec<f64> = pts
            .windows(2)
            .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
            .collect();
        let mut half = lens.iter().sum::<f64>() / 2.0;
        for (w, len) in pts.windows(2).zip(&lens) {
            if half <= *len && *len > 0.0 {
                let t = half / len;
                return Some((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
            }
            half -= len;
        }
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(code: &str, polyline: Vec<(i64, i64)>) -> WireStroke {
        WireStroke::new(ew_api::output::Uuid::nil(), polyline, code)
    }

    #[test]
    fn the_codes_give_their_colours() {
        assert_eq!(colours_of("BU"), [(0x1F, 0x5F, 0xD6)]);
        assert_eq!(colours_of("GNYE"), [(0x1A, 0x9A, 0x3B), (0xFF, 0xD7, 0x00)]);
        assert_eq!(colours_of("DBU"), [(0x11, 0x34, 0x76)]);
        assert_eq!(colours_of("LBU"), [(0x8F, 0xAF, 0xEA)]);
        assert_eq!(colours_of("XYZ"), [UNKNOWN_RGB]);
        assert_eq!(colours_of(""), [UNKNOWN_RGB]);
        assert_eq!(colours_of("é"), [UNKNOWN_RGB]);
        assert!(stroke("WH", vec![]).cased && stroke("YE", vec![]).cased);
        assert!(!stroke("GNYE", vec![]).cased && !stroke("DBU", vec![]).cased);
    }

    #[test]
    fn a_two_colour_wire_alternates_2_mm_dashes_over_its_corners() {
        let s = stroke("GNYE", vec![(0, 0), (3_000_000, 0), (3_000_000, 2_000_000)]);
        let p = s.pieces();
        assert_eq!(
            p,
            [
                ((0.0, 0.0), (2_000_000.0, 0.0), 0),
                ((2_000_000.0, 0.0), (3_000_000.0, 0.0), 1),
                ((3_000_000.0, 0.0), (3_000_000.0, 1_000_000.0), 1),
                ((3_000_000.0, 1_000_000.0), (3_000_000.0, 2_000_000.0), 0),
            ]
        );
        let one = stroke("DBU", vec![(0, 0), (0, 0), (5, 0)]);
        assert_eq!(one.pieces(), [((0.0, 0.0), (5.0, 0.0), 0)]);
    }

    #[test]
    fn the_code_stands_at_the_middle_of_the_path() {
        let s = stroke("BU", vec![(0, 0), (4, 0), (4, 4)]);
        assert_eq!(s.midpoint(), Some((4.0, 0.0)));
        assert_eq!(stroke("BU", vec![(1, 1)]).midpoint(), Some((1.0, 1.0)));
        assert_eq!(stroke("BU", vec![]).midpoint(), None);
    }
}
