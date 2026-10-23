// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company logo through the export job (FT-384): an SVG logo reaches every writer as vector paths (no image item
//! is left to skip); a PNG logo is embedded by the SVG writer as an `<image>` and counted as skipped by the PDF writer
//! (krilla's raster support is a dependency decision still open). Golden A's fixture with placeholder logos.

use std::path::Path;

use ew_api::output::{ExportOptions, PageSelection};
use ew_api::{AnyCommand, Session};

fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

fn set_logo(s: &mut Session, bytes: &[u8]) {
    let cmd = AnyCommand {
        name: "project.company.set".into(),
        params: serde_json::json!({ "logo": ew_api::base64_encode(bytes) }),
    };
    let p = s.preview(&cmd).unwrap();
    s.commit(p).unwrap();
}

fn cover() -> ExportOptions {
    ExportOptions {
        pages: PageSelection::Folios(vec!["1".to_owned()]),
        ..ExportOptions::default()
    }
}

/// A PNG of `w × h` opaque red pixels (synthetic).
fn red_png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&[255, 0, 0, 255].repeat((w * h) as usize))
        .unwrap();
    writer.finish().unwrap();
    out
}

#[test]
fn an_svg_logo_is_drawn_as_paths_by_every_writer() {
    let mut s = golden();
    let plain_svg = s.export_svg(&cover()).unwrap();
    set_logo(
        &mut s,
        br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 10"><rect width="40" height="10" fill="#123456"/></svg>"##,
    );
    let svg = s.export_svg(&cover()).unwrap();
    assert_eq!(svg.report.skipped_images, 0);
    assert_eq!(svg.report.images, 0);
    let page = svg.files[0].text.clone();
    assert!(page.contains("#123456"), "the logo's paths in the SVG page");
    assert!(!plain_svg.files[0].text.contains("#123456"));
    let pdf = s.export_pdf(&cover()).unwrap();
    assert_eq!(
        pdf.report.skipped_images, 0,
        "no image item is left for the PDF writer"
    );
}

#[test]
fn a_png_logo_is_embedded_by_the_svg_writer_and_counted_by_the_pdf_writer() {
    let mut s = golden();
    let png = red_png(8, 2);
    set_logo(&mut s, &png);
    let svg = s.export_svg(&cover()).unwrap();
    assert_eq!((svg.report.images, svg.report.skipped_images), (1, 0));
    let page = svg.files[0].text.clone();
    assert!(
        page.contains(&format!(
            "href=\"data:image/png;base64,{}\"",
            ew_api::base64_encode(&png)
        )),
        "the PNG's bytes embedded"
    );
    let pdf = s.export_pdf(&cover()).unwrap();
    assert_eq!(
        pdf.report.skipped_images, 1,
        "the PDF writer does not embed rasters yet"
    );
}
