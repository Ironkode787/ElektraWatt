// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application icon (O5): `assets/icons/app/elektrawatt.png` is the
//! rendering of `assets/icons/app/elektrawatt.svg` at 256 × 256 pixels, RGBA with transparent corners, through the
//! workspace's own SVG path (`ew_scene::images::svg_items`, usvg) and CPU renderer (`ew-render-cpu`, vello_cpu), on
//! a transparent background. The test renders the SVG and compares the PNG's bytes; `EW_BLESS=1` writes the PNG.
//! The same SVG, the menu button's mark, is the Windows icon `elektrawatt.ico` at six sizes (FT-1710).

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
const ICO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icons/app/elektrawatt.ico"
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

/// The icon rendered at `side` × `side` pixels (one pixel 0.1 mm at 254 dpi), as the check renders it.
fn render_image(side: u32) -> ew_render_cpu::RgbaImage {
    let svg = std::fs::read(SVG).expect("the icon's SVG");
    let side_nm = Nm(i64::from(side) * SIDE.0 / 256);
    let rect = Rect::from_points(Pt2::ORIGIN, Pt2::new(side_nm, side_nm));
    let drawn = ew_scene::images::svg_items(&svg, rect).expect("the icon's SVG parses");
    assert_eq!(drawn.skipped, 0, "every node of the icon's SVG is drawn");
    let list = DisplayList { items: drawn.items };
    let options = RenderOptions {
        dpi: DPI,
        background: Color::from_rgba8(0, 0, 0, 0),
    };
    let sheet = SheetSize {
        width: side_nm,
        height: side_nm,
    };
    let (image, report) =
        ew_render_cpu::render(&list, sheet, &options, &NoFonts, None).expect("the icon's pixels");
    assert_eq!((image.width, image.height), (side, side));
    assert_eq!(report, ew_render_cpu::RenderReport::default());
    image
}

/// The PNG of the SVG at 256 × 256 pixels.
fn render() -> Vec<u8> {
    let image = render_image(256);
    // The rounded square leaves the corners transparent; its middle is the accent, volt-600.
    assert_eq!(image.pixels[..4], [0, 0, 0, 0]);
    let at = |x: usize, y: usize| image.pixels[(y * 256 + x) * 4..][..4].to_vec();
    assert_eq!(at(40, 128), [0x0B, 0x63, 0xCE, 0xFF]);
    assert_eq!(at(136, 84), [0xFF, 0xFF, 0xFF, 0xFF]);
    ew_render_cpu::encode_png(&image, None)
}

/// The sizes of the Windows icon (FT-1710): Explorer's and the task bar's at 100 % to 200 %, and the large view.
const ICO_SIDES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// The Windows icon of the SVG: an ICO file whose images are the SVG rendered at [`ICO_SIDES`], each stored as a PNG
/// (the form Windows reads since Vista).
fn render_ico() -> Vec<u8> {
    let images: Vec<(u32, Vec<u8>)> = ICO_SIDES
        .iter()
        .map(|&side| (side, ew_render_cpu::encode_png(&render_image(side), None)))
        .collect();
    let count = u16::try_from(images.len()).unwrap();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (side, png) in &images {
        // A side of 256 is written as 0.
        let byte = u8::try_from(*side % 256).unwrap();
        ico.extend_from_slice(&[byte, byte, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        let len = u32::try_from(png.len()).unwrap();
        ico.extend_from_slice(&len.to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }
    for (_, png) in &images {
        ico.extend_from_slice(png);
    }
    ico
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

/// FT-1710: the program's Windows icon (`elektrawatt.ico`, which `build.rs` embeds in `elektrawatt.exe` and the setup
/// shows) is the same SVG, the menu button's mark, rendered at each size of [`ICO_SIDES`].
#[test]
fn the_windows_icon_is_the_rendering_of_the_same_svg() {
    let ico = render_ico();
    if bless() {
        std::fs::write(ICO, &ico).expect("write the icon's ICO");
        return;
    }
    let committed = std::fs::read(ICO).expect("the icon's ICO");
    assert!(
        committed == ico,
        "assets/icons/app/elektrawatt.ico is not the rendering of elektrawatt.svg: run this test with EW_BLESS=1"
    );
}
