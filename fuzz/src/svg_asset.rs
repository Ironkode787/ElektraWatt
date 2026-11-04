// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The SVG asset target (WP-S6.04 D9; doc 18 §11 TM-18): bytes through the asset store's sniff
//! (`ew_schema::sniff`, which runs `svg_hazard` on an SVG); what it accepts as an SVG is drawn through `ew-scene`'s
//! path (`ew_scene::images::svg_items`: the hazard check again, then usvg inside `catch_unwind`). Property: whatever
//! the bytes, nothing panics.

use ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_schema::{AssetFormat, sniff};

/// What one input did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Outcome {
    /// Whether the sniff accepted the bytes as an SVG.
    pub svg: bool,
    /// Whether `ew-scene` drew it.
    pub drawn: bool,
}

/// Runs one input.
pub fn check(data: &[u8]) -> Outcome {
    let Ok((AssetFormat::Svg, _)) = sniff(data) else {
        return Outcome {
            svg: false,
            drawn: false,
        };
    };
    let rect = Rect::from_points(
        Pt2::new(Nm::from_mm(10), Nm::from_mm(10)),
        Pt2::new(Nm::from_mm(110), Nm::from_mm(80)),
    );
    Outcome {
        svg: true,
        drawn: ew_scene::images::svg_items(data, rect).is_ok(),
    }
}
