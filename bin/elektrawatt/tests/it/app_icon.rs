// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application icon (WP-S6.06 §13 O5, decision log ID-1280): `assets/icons/app/elektrawatt.png` is the
//! rendering of `assets/icons/app/elektrawatt.svg` at 256 × 256 pixels, RGBA with transparent corners, through the
//! workspace's own SVG path (`ew_scene::images::svg_items`, usvg) and CPU renderer (`ew-render-cpu`, vello_cpu), on
//! a transparent background. The test renders the SVG and compares the PNG's bytes; `EW_BLESS=1` writes the PNG.

use ew_render_cpu::ew_dl::ew_units::{Nm, Pt2, Rect};
use ew_render_cpu::ew_dl::{Color, DisplayList, FontKey};
use ew_render_cpu::{FontBytes, RenderOptions, SheetSize};

const SVG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icons/app/elektrawatt.svg"
);
const PNG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icons/app/elektrawatt.png"
);

/// The icon's side: 256 pixels at 254 dpi, one pixel 0.1 mm.
const SIDE: Nm = Nm(25_600_000);
const DPI: u32 = 254;

/// No glyphs: the icon is two paths.
struct NoFonts;

impl ew_render_cpu::FontResolver for NoFonts {
    fn font(&self, _key: FontKey) -> Option<FontBytes> {
        None
    }
}

// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The PNG of the SVG, rendered as the check renders it.
fn render() -> Vec<u8> {
    let svg = std::fs::read(SVG).expect("the icon's SVG");
    let rect = Rect::from_points(Pt2::ORIGIN, Pt2::new(SIDE, SIDE));
    let drawn = ew_scene::images::svg_items(&svg, rect).expect("the icon's SVG parses");
    assert_eq!(drawn.skipped, 0, "every node of the icon's SVG is drawn");
    let list = DisplayList { items: drawn.items };
    let options = RenderOptions {
        dpi: DPI,
        background: Color::from_rgba8(0, 0, 0, 0),
    };
    let sheet = SheetSize {
        width: SIDE,
        height: SIDE,
    };
    let (image, report) =
        ew_render_cpu::render(&list, sheet, &options, &NoFonts, None).expect("256 × 256 pixels");
    assert_eq!((image.width, image.height), (256, 256));
    assert_eq!(report, ew_render_cpu::RenderReport::default());
    // The rounded square leaves the corners transparent; its middle is the accent, volt-600.
    assert_eq!(image.pixels[..4], [0, 0, 0, 0]);
    let at = |x: usize, y: usize| image.pixels[(y * 256 + x) * 4..][..4].to_vec();
    assert_eq!(at(40, 128), [0x0B, 0x63, 0xCE, 0xFF]);
    assert_eq!(at(136, 84), [0xFF, 0xFF, 0xFF, 0xFF]);
    ew_render_cpu::encode_png(&image, None)
}

#[test]
fn the_application_icon_png_is_the_rendering_of_its_svg() {
    let png = render();
    if bless() {
        std::fs::write(PNG, &png).expect("write the icon's PNG");
        return;
    }
    let committed = std::fs::read(PNG).expect("the icon's PNG");
    assert!(
        committed == png,
        "assets/icons/app/elektrawatt.png is not the rendering of elektrawatt.svg: run this test with EW_BLESS=1"
    );
}
