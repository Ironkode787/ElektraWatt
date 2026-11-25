// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A selected line (a wire, a fluid line) marks its two ends with the connection point's ring (docs/design/16a-design-
//! language.md §7.1), and a line that is not selected marks none.

use std::collections::BTreeSet;

use ew_render_cpu::SheetSize;
use ew_render2d::overlay::{Bands, OpenCps, OverlayInput, build};
use ew_render2d::view::{PxRect, View};
use ew_units::Nm;

fn pixels(rects: &[PxRect]) -> BTreeSet<(i64, i64)> {
    rects
        .iter()
        .flat_map(|r| (r.y0..r.y1).flat_map(move |y| (r.x0..r.x1).map(move |x| (x, y))))
        .collect()
}

#[test]
fn a_selected_line_rings_its_two_ends() {
    let view = View {
        centre: (148_500_000, 105_000_000),
        zoom: 1.0,
        pane: (1200.0, 800.0),
        ppp: 1.0,
    };
    let t = view.settled_transform();
    let line = vec![
        (100_000_000, 80_000_000),
        (160_000_000, 80_000_000),
        (160_000_000, 120_000_000),
    ];
    let selected = [line.clone()];
    let overlay = |selected: &[Vec<(i64, i64)>]| {
        build(&OverlayInput {
            transform: t,
            pane_px: view.pane_px_whole(),
            ppp: 1.0,
            sheet: SheetSize {
                width: Nm::from_mm(297),
                height: Nm::from_mm(210),
            },
            grid: None,
            hover: None,
            selected: &[],
            cursor: None,
            tool: &[],
            open_cps: OpenCps::default(),
            bands: Bands {
                hover: Some(&line),
                selected,
                radius_pt: 4.0,
            },
        })
    };
    let px = |p: (i64, i64)| {
        let (x, y) = t.to_screen((p.0 as f64, p.1 as f64));
        (x.floor() as i64, y.floor() as i64)
    };
    let rings = pixels(&overlay(&selected).markers);
    for end in [line[0], line[2]] {
        let c = px(end);
        // The 9-pixel ring of a connection point, open at its centre.
        assert!(
            rings.contains(&(c.0 + 4, c.1)) && rings.contains(&(c.0, c.1 - 4)),
            "{end:?}"
        );
        assert!(!rings.contains(&c), "{end:?}");
    }
    // The bend is no end.
    let bend = px(line[1]);
    assert!(!rings.contains(&(bend.0 + 4, bend.1)));
    // Hovered only, the line marks no end.
    assert!(overlay(&[]).markers.is_empty());
}
