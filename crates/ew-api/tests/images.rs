// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company logo through the export job (FT-384, FT-406): an SVG logo reaches every writer as vector paths (no image
//! item is left to skip); a PNG or JPEG logo is embedded by the SVG writer as an `<image>` and by the PDF writer as an
//! image XObject (the JPEG unchanged). Golden A's fixture with placeholder logos, in a read-only session (a test copy:
//! nothing is written back).

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

/// A 16 × 8 baseline JPEG (4:4:4, all quantisers 1, DC-only blocks, written by hand): the left 8 × 8 block red, the
/// right one blue.
fn red_blue_jpeg() -> Vec<u8> {
    let mut b = vec![0xff, 0xd8, 0xff, 0xdb, 0x00, 0x84, 0x00];
    b.extend([1u8; 64]);
    b.push(0x01);
    b.extend([1u8; 64]);
    b.extend_from_slice(&[
        0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x08, 0x00, 0x10, 0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03,
        0x11, 0x01, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a,
        0x0b, 0xff, 0xc4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00,
        0x00, 0x3f, 0x00, 0x92, 0xfa, 0x54, 0xea, 0xfe, 0x12, 0x87, 0x5d, 0x50, 0x5b, 0x5f, 0x7f, 0xff, 0xd9,
    ]);
    b
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

#[test]
fn a_png_logo_is_embedded_by_the_svg_writer_and_the_pdf_writer() {
    let mut s = golden();
    let plain = s.export_pdf(&cover()).unwrap();
    assert_eq!((plain.report.images, plain.report.skipped_images), (0, 0));
    assert_eq!(
        count(&plain.bytes, b"/Subtype/Image"),
        0,
        "the golden cover has no raster"
    );
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
    assert_eq!((pdf.report.images, pdf.report.skipped_images), (1, 0));
    assert_eq!(count(&pdf.bytes, b"/Subtype/Image"), 1, "one image XObject");
    assert_eq!(count(&pdf.bytes, b"/Width 8/Height 2"), 1, "the PNG's size");
    assert!(
        !pdf.findings
            .iter()
            .any(|f| matches!(f.finding, ew_scene::page::PageFinding::Image(_))),
        "no image finding"
    );
}

/// FT-406: a JPEG logo prints on the golden cover in the PDF, embedded unchanged (DCT); the SVG writer embeds it too.
#[test]
fn a_jpeg_logo_is_printed_on_the_cover_in_the_pdf() {
    let mut s = golden();
    let jpeg = red_blue_jpeg();
    set_logo(&mut s, &jpeg);
    let pdf = s.export_pdf(&cover()).unwrap();
    assert_eq!((pdf.report.images, pdf.report.skipped_images), (1, 0));
    assert_eq!(count(&pdf.bytes, b"/Subtype/Image"), 1);
    assert_eq!(
        count(&pdf.bytes, b"/Filter/DCTDecode/Width 16/Height 8"),
        1,
        "the JPEG unchanged"
    );
    assert_eq!(count(&pdf.bytes, &jpeg), 1, "the JPEG's own bytes");
    let svg = s.export_svg(&cover()).unwrap();
    assert_eq!((svg.report.images, svg.report.skipped_images), (1, 0));
    assert!(svg.files[0].text.contains("data:image/jpeg;base64,"));
    // The same export twice: equal bytes.
    assert_eq!(s.export_pdf(&cover()).unwrap().bytes, pdf.bytes);
}

/// Review fix 8: an image the export cannot draw is reported, not dropped silently: a logo whose SVG usvg refuses (an
/// unbalanced document the store admits), and a logo whose bytes the export was not given (the free function, which
/// holds no asset bytes), each as a page finding of the cover.
#[test]
fn logos_that_cannot_be_drawn_are_page_findings() {
    use ew_api::output::{PageFindingAt, export_pages};
    use ew_scene::images::ImageProblem;
    use ew_scene::page::PageFinding;
    let image_findings = |findings: &[PageFindingAt]| -> Vec<(String, ImageProblem)> {
        findings
            .iter()
            .filter_map(|f| match &f.finding {
                PageFinding::Image(issue) => Some((f.folio.clone(), issue.problem.clone())),
                _ => None,
            })
            .collect()
    };
    let mut s = golden();
    assert!(image_findings(&s.export_pages(&cover()).unwrap().findings).is_empty());
    set_logo(
        &mut s,
        br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><g><rect width="4" height="4"/></svg>"#,
    );
    let found = image_findings(&s.export_pages(&cover()).unwrap().findings);
    assert!(
        matches!(found.as_slice(), [(folio, ImageProblem::Refused(_))] if folio == "1"),
        "{found:?}"
    );
    let mut s = golden();
    set_logo(&mut s, &red_png(2, 2));
    let found = image_findings(&export_pages(&s.query(), &cover()).unwrap().findings);
    assert_eq!(found, [("1".to_owned(), ImageProblem::Missing)]);
}

/// Review item 1 (FT-406): a JPEG logo that passes the store's sniff (its `SOF` header is intact) but whose data is cut
/// does not vanish silently: the export reports it as a page finding of the cover (`ImageProblem::Undecodable`) and
/// the PDF writer skips it.
#[test]
fn a_raster_logo_that_does_not_decode_is_a_page_finding() {
    use ew_scene::images::ImageProblem;
    use ew_scene::page::PageFinding;
    let jpeg = red_blue_jpeg();
    let cut = &jpeg[..jpeg.len() - 12];
    let mut s = golden();
    set_logo(&mut s, cut);
    let pdf = s.export_pdf(&cover()).unwrap();
    let found: Vec<(String, ImageProblem, String)> = pdf
        .findings
        .iter()
        .filter_map(|f| match &f.finding {
            PageFinding::Image(issue) => Some((f.folio.clone(), issue.problem.clone(), issue.to_string())),
            _ => None,
        })
        .collect();
    let hex: String = <sha2::Sha256 as sha2::Digest>::digest(cut)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let message = format!(
        "the image sha256:{hex} is not drawn: its JPEG bytes do not decode (corrupt, truncated, unsupported or larger \
         than 16 Mi pixels); export it again as a plain PNG or baseline JPEG"
    );
    assert_eq!(
        found,
        [(
            "1".to_owned(),
            ImageProblem::Undecodable(ew_dl::ImageFormat::Jpeg),
            message
        )]
    );
    assert_eq!((pdf.report.images, pdf.report.skipped_images), (0, 1));
    // The SVG of the same export does not carry the bytes either (the PDF writer's rule), and reports the same finding.
    let svg = s.export_svg(&cover()).unwrap();
    assert_eq!((svg.report.images, svg.report.skipped_images), (0, 1));
    assert!(!svg.files[0].text.contains("<image"), "no <image> in the SVG");
    assert!(
        svg.findings
            .iter()
            .any(|f| matches!(&f.finding, PageFinding::Image(i) if i.problem == ImageProblem::Undecodable(ew_dl::ImageFormat::Jpeg)))
    );
}
