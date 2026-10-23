// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! SVG structure, links, determinism and parallelism of the writer (doc 09 §4.4; FR-OUT-013, FR-OUT-019;
//! AC-14 in miniature).
//!
//! No XML parser is in the dependency graph, so the checks read the documents textually.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_dl::{Brush, Color, Destination, DisplayList, FontKey, Item, Link, Path, Pen, Stroke, Transform};
use ew_export::{Parallelism, SvgError, SvgFace, SvgOptions, SvgOutput, par_map, svg, svg_file_name, svg_id};
use ew_text::{builtin, bundled};
use ew_units::{Nm, Orient2, Pt2, Rect};
use uuid::Uuid;

fn faces() -> BTreeMap<FontKey, SvgFace> {
    bundled()
        .faces()
        .map(|f| {
            (
                f.key,
                SvgFace {
                    family: f.family_name.clone(),
                    weight: f.weight.value(),
                    italic: f.italic,
                },
            )
        })
        .collect()
}

fn options() -> SvgOptions {
    SvgOptions {
        faces: faces(),
        ..SvgOptions::default()
    }
}

fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

fn run(s: &str, at: Pt2) -> ew_dl::ShapedRun {
    let style = builtin().resolve("text.normal").unwrap();
    let mut run = (*bundled().shape(s, &style, None).run).clone();
    run.origin = at;
    run
}

fn text(s: &str, at: Pt2) -> Item {
    Item::glyphs(Arc::new(run(s, at)))
}

fn page(n: u128, label: &str, items: Vec<Item>) -> ew_export::PdfPage {
    ew_export::PdfPage {
        destination: Destination::Page(Uuid::from_u128(n)),
        label: label.to_owned(),
        title: format!("{label} Page {label}"),
        section: None,
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList { items },
    }
}

fn rep(n: u128) -> Destination {
    Destination::Representation(Uuid::from_u128(n))
}

fn line(y: i64, stroke: Stroke) -> Item {
    Item::path(Path::line(mm(10, y), mm(200, y)), Some(stroke), None)
}

/// The text of a one-page export of `items`.
fn one(items: Vec<Item>, options: &SvgOptions) -> (String, ew_export::SvgReport) {
    let out = svg(&[page(1, "1", items)], options, None, Parallelism::Serial).unwrap();
    (out.files[0].text.clone(), out.report)
}

/// Three pages: "1"; "101" (sources to a target on "103" and to one outside the export); "103" (the
/// target twice, a source to its own page).
fn pages() -> Vec<ew_export::PdfPage> {
    let target = Item::path(
        Path::circle(mm(100, 80), Nm::from_mm(2)),
        None,
        Some(Brush::BLACK),
    )
    .with_link(Link::Target(rep(7)));
    vec![
        page(1, "1", vec![text("Title Page", mm(20, 20))]),
        page(
            101,
            "101",
            vec![
                text("103-G6", mm(50, 50)).with_link(Link::Source(rep(7))),
                text("206-A1", mm(50, 60)).with_link(Link::Source(rep(99))),
                text("SK 3110.000", mm(30, 100)),
            ],
        ),
        page(
            103,
            "103",
            vec![
                target.clone(),
                target,
                line(150, Stroke::pen(Pen::P026))
                    .with_link(Link::Source(Destination::Page(Uuid::from_u128(103)))),
            ],
        ),
    ]
}

/// Every `href` whose file is not in the export or whose fragment is not an `id` of that file, and every id
/// written twice in one file.
fn dangling(out: &SvgOutput) -> Vec<String> {
    let mut bad = Vec::new();
    for f in &out.files {
        for rest in f.text.split("href=\"").skip(1) {
            let h = &rest[..rest.find('"').unwrap()];
            let (file, fragment) = h.split_once('#').map_or((h, None), |(a, b)| (a, Some(b)));
            match out.files.iter().find(|g| g.name == file) {
                None => bad.push(format!("{}: {h} (no such file)", f.name)),
                Some(g) => {
                    if let Some(id) = fragment
                        && count(&g.text, &format!(" id=\"{id}\"")) != 1
                    {
                        bad.push(format!("{}: {h} (no such id)", f.name));
                    }
                }
            }
        }
        for rest in f.text.split(" id=\"").skip(1) {
            let id = &rest[..rest.find('"').unwrap()];
            if count(&f.text, &format!(" id=\"{id}\"")) != 1 {
                bad.push(format!("{}: id {id} written twice", f.name));
            }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// The value of the attribute `name` of the first `<tag` element.
fn attr<'a>(doc: &'a str, tag: &str, name: &str) -> &'a str {
    let el = &doc[doc.find(&format!("<{tag}")).unwrap()..];
    let el = &el[..el.find('>').unwrap()];
    let key = format!(" {name}=\"");
    let i = el.find(&key).unwrap() + key.len();
    let rest = &el[i..];
    &rest[..rest.find('"').unwrap()]
}

#[test]
fn structure_files_names_order_root_ids_titles() {
    let out = svg(&pages(), &options(), None, Parallelism::Serial).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["1.svg", "101.svg", "103.svg"]);
    for (f, n) in out.files.iter().zip([1u128, 101, 103]) {
        let t = &f.text;
        assert!(
            t.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
            "{}",
            f.name
        );
        assert!(t.contains("width=\"297mm\" height=\"210mm\" viewBox=\"0 0 297 210\""));
        let id = svg_id(&Destination::Page(Uuid::from_u128(n)));
        assert!(id.starts_with("ew-p-"));
        assert!(t.contains(&format!("id=\"{id}\"")), "root id of {}", f.name);
        let label = f.name.trim_end_matches(".svg");
        assert!(t.contains(&format!("<title>{label} Page {label}</title>")));
        assert!(t.ends_with("</svg>\n"));
    }
}

#[test]
fn text_is_written_as_text_with_one_x_per_character() {
    let opts = options();
    let (doc, report) = one(vec![text("SK 3110.000", mm(30, 100))], &opts);
    assert_eq!(report.texts, 1);
    assert_eq!(count(&doc, "<text"), 1);
    assert!(doc.contains(">SK 3110.000</text>"), "the space is kept");
    assert_eq!(attr(&doc, "text", "x").split(' ').count(), 11);
    let key = run("A", Pt2::ORIGIN).font;
    assert_eq!(
        attr(&doc, "text", "font-family"),
        format!("{}, sans-serif", opts.faces[&key].family)
    );
    // Markup in the text is escaped.
    let (doc, _) = one(vec![text("<&>", mm(30, 100))], &opts);
    assert!(doc.contains(">&lt;&amp;&gt;</text>"));
}

#[test]
fn links_sources_targets_plain_text_and_page_links() {
    let out = svg(&pages(), &options(), None, Parallelism::Serial).unwrap();
    let id = svg_id(&rep(7));
    assert!(id.starts_with("ew-r-"));
    let (p101, p103) = (&out.files[1].text, &out.files[2].text);
    assert!(p101.contains(&format!("<a href=\"103.svg#{id}\">")));
    assert_eq!(count(p101, "<a "), 1, "the source outside the export has no <a>");
    // Two items carry the target; the id is written once.
    assert_eq!(count(p103, &format!("id=\"{id}\"")), 1);
    assert!(p103.contains("<a href=\"103.svg\">"));
    assert_eq!(out.report.links, 2);
    assert_eq!(out.report.plain_text_links, 1);
    let anchors: usize = out.files.iter().map(|f| count(&f.text, "<a ")).sum();
    assert_eq!(anchors, out.report.links);
    assert!(!out.files.iter().any(|f| f.text.contains(&svg_id(&rep(99)))));
    assert_eq!(dangling(&out), Vec::<String>::new());
}

/// A target whose first item draws nothing (a path with neither stroke nor fill, an unresolved instance)
/// gets its id on the first item that is drawn, and links point at the first page that draws it, so no
/// link dangles.
#[test]
fn a_target_id_is_written_only_on_a_drawn_item_and_every_link_resolves() {
    let blank = Item::path(Path::line(mm(10, 10), mm(20, 10)), None, None).with_link(Link::Target(rep(7)));
    let unresolved = Item::instance(
        ew_dl::SymbolGeomId {
            symbol: Uuid::from_u128(5),
            variant: Orient2::R0,
            extra: None,
        },
        Transform::IDENTITY,
    )
    .with_link(Link::Target(rep(7)));
    let drawn = line(40, Stroke::pen(Pen::P026)).with_link(Link::Target(rep(7)));
    let other = line(50, Stroke::pen(Pen::P026)).with_link(Link::Target(rep(8)));
    let pages = vec![
        page(
            101,
            "101",
            vec![
                text("102-A1", mm(50, 50)).with_link(Link::Source(rep(7))),
                text("102-A2", mm(50, 60)).with_link(Link::Source(rep(8))),
            ],
        ),
        page(102, "102", vec![blank.clone(), unresolved.clone()]),
        page(103, "103", vec![blank.clone(), unresolved, drawn, other.clone()]),
        page(104, "104", vec![blank, other]),
    ];
    let out = svg(&pages, &options(), None, Parallelism::threads(3)).unwrap();
    let (id7, id8) = (svg_id(&rep(7)), svg_id(&rep(8)));
    assert!(out.files[0].text.contains(&format!("<a href=\"103.svg#{id7}\">")));
    assert!(out.files[0].text.contains(&format!("<a href=\"103.svg#{id8}\">")));
    assert!(
        !out.files[1].text.contains(&id7),
        "page 102 draws nothing of the target"
    );
    let p103 = &out.files[2].text;
    assert_eq!(count(p103, &format!(" id=\"{id7}\"")), 1);
    let at = p103.find(&format!(" id=\"{id7}\"")).unwrap();
    assert!(p103[..at].ends_with("<path"), "the id is on the drawn path");
    assert_eq!(dangling(&out), Vec::<String>::new());
    assert_eq!(out, svg(&pages, &options(), None, Parallelism::Serial).unwrap());
}

#[test]
fn the_dangling_check_sees_a_missing_file_and_a_missing_id() {
    let mut out = svg(&pages(), &options(), None, Parallelism::Serial).unwrap();
    assert!(dangling(&out).is_empty());
    let id = svg_id(&rep(7));
    out.files[2].text = out.files[2].text.replace(&format!(" id=\"{id}\""), "");
    out.files[1].text.push_str("<a href=\"999.svg\"></a>");
    let bad = dangling(&out);
    assert_eq!(bad.len(), 2, "{bad:?}");
}

#[test]
fn output_is_deterministic_and_independent_of_the_thread_count() {
    let opts = options();
    let serial: SvgOutput = svg(&pages(), &opts, None, Parallelism::Serial).unwrap();
    assert_eq!(svg(&pages(), &opts, None, Parallelism::Serial).unwrap(), serial);
    for n in [1, 2, 3, 8] {
        let p = svg(&pages(), &opts, None, Parallelism::threads(n)).unwrap();
        assert_eq!(p.files, serial.files, "{n} threads");
        assert_eq!(p.report, serial.report, "{n} threads");
    }
    assert!(dangling(&serial).is_empty());
    let items: Vec<u64> = (0..50).collect();
    let f = |i: usize, x: &u64| (i, x * 3 + 1);
    assert_eq!(
        par_map(&items, Parallelism::threads(7), f),
        par_map(&items, Parallelism::Serial, f)
    );
}

#[test]
fn errors_no_pages_duplicate_names_page_size() {
    let opts = options();
    assert_eq!(
        svg(&[], &opts, None, Parallelism::Serial).unwrap_err(),
        SvgError::NoPages
    );
    let dup = [page(1, "1", vec![]), page(2, "1", vec![])];
    assert_eq!(
        svg(&dup, &opts, None, Parallelism::Serial).unwrap_err(),
        SvgError::DuplicateFile {
            index: 1,
            name: "1.svg".into()
        }
    );
    let mut flat = page(1, "1", vec![]);
    flat.width = Nm::ZERO;
    assert_eq!(
        svg(&[flat], &opts, None, Parallelism::Serial).unwrap_err(),
        SvgError::PageSize { index: 0 }
    );
    assert_eq!(svg_file_name("A/B C"), "A_B_C.svg");
}

#[test]
fn a_run_without_a_face_is_not_drawn_and_reported_once() {
    let a = text("Hello", mm(10, 10));
    let b = text("World", mm(10, 20));
    let key = run("Hello", Pt2::ORIGIN).font;
    let (doc, report) = one(vec![a, b], &SvgOptions::default());
    assert!(!doc.contains("<text"));
    assert_eq!(report.texts, 0);
    assert_eq!(report.missing_fonts, vec![key]);
}

#[test]
fn paint_dashes_alpha_clips_and_transforms() {
    let opts = options();
    let dashed = line(20, Stroke::pen(Pen::P026).dashed(ew_dl::DashPattern::Dash42));
    let plain = line(30, Stroke::pen(Pen::P026));
    let (doc, _) = one(vec![dashed], &opts);
    assert!(doc.contains("stroke-dasharray="));
    let (doc, _) = one(vec![plain], &opts);
    assert!(!doc.contains("stroke-dasharray="));

    let translucent = Item::path(
        Path::rect(Rect::from_points(mm(30, 160), mm(60, 180))),
        None,
        Some(Brush::Solid(Color {
            r: 255,
            g: 0,
            b: 0,
            a: 128,
        })),
    );
    let (doc, _) = one(vec![translucent], &opts);
    assert!(doc.contains("fill=\"#ff0000\""));
    assert!(doc.contains("fill-opacity="));

    let clipped = Item::group(
        Some(Transform {
            translate: mm(5, 6),
            ..Transform::IDENTITY
        }),
        Some(Rect::from_points(mm(0, 0), mm(50, 40))),
        vec![line(10, Stroke::pen(Pen::P026))],
    );
    let (doc, _) = one(vec![clipped], &opts);
    assert!(doc.contains("<clipPath id=\"c0-1\">"));
    assert!(doc.contains("clip-path=\"url(#c0-1)\""));
    assert!(doc.contains("transform=\"translate(5 6)\""));

    let turned = Item::group(
        Some(Transform {
            translate: mm(5, 6),
            ..Transform::from_orient(Orient2::R90)
        }),
        None,
        vec![line(10, Stroke::pen(Pen::P026))],
    );
    let (doc, _) = one(vec![turned], &opts);
    assert!(doc.contains("transform=\"matrix(0 1 -1 0 5 6)\""), "{doc}");
    assert!(!doc.contains("translate("));
}

#[test]
fn monochrome_turns_strokes_black_and_fills_grey() {
    let red = Brush::Solid(Color::from_rgb8(255, 0, 0));
    let items = vec![
        line(20, Stroke::pen(Pen::P026).with_brush(red)),
        Item::path(
            Path::rect(Rect::from_points(mm(30, 160), mm(60, 180))),
            None,
            Some(red),
        ),
    ];
    let (colour, _) = one(items.clone(), &options());
    assert!(colour.contains("stroke=\"#ff0000\""));
    let mono = SvgOptions {
        monochrome: true,
        ..options()
    };
    let (doc, _) = one(items, &mono);
    assert!(doc.contains("stroke=\"#000000\""));
    // Luminance grey of pure red: (2126 * 255 + 5000) / 10000 = 54 = 0x36.
    assert!(doc.contains("fill=\"#363636\""));
    assert!(!doc.contains("#ff0000"));
}

#[test]
fn ten_thousand_paths_are_written() {
    let items: Vec<Item> = (0..10_000i64)
        .map(|i| {
            let (x, y) = (i % 250, i / 250);
            Item::path(
                Path::line(mm(x, y), mm(x + 1, y + 1)),
                Some(Stroke::pen(Pen::P026)),
                None,
            )
        })
        .collect();
    let (doc, _) = one(items, &options());
    assert_eq!(count(&doc, "<path"), 10_000);
    assert!(doc.ends_with("</svg>\n"));
}

fn image_item(sha: u8, format: ew_dl::ImageFormat, bytes: &[u8]) -> Item {
    Item::image(
        Arc::new(ew_dl::ImageData {
            sha256: [sha; 32],
            format,
            bytes: Arc::from(bytes),
        }),
        Rect::from_points(mm(10, 20), mm(50, 30)),
    )
}

/// FT-384: PNG and JPEG images are written as `<image>` elements with their bytes embedded in base64, stretched over
/// their rectangle; an image without bytes, an SVG image (which reaches the writer as paths) and bytes whose magic
/// number is not their format's are counted as skipped.
#[test]
fn png_and_jpeg_images_are_embedded() {
    let png = b"\x89PNG\r\n\x1a\nrest";
    let jpeg = [0xff, 0xd8, 0xff, 0xe0, 1, 2];
    let (doc, report) = one(
        vec![
            image_item(1, ew_dl::ImageFormat::Png, png),
            image_item(2, ew_dl::ImageFormat::Jpeg, &jpeg),
            image_item(3, ew_dl::ImageFormat::Png, &[]),
            image_item(4, ew_dl::ImageFormat::Svg, b"<svg/>"),
            image_item(5, ew_dl::ImageFormat::Jpeg, png),
        ],
        &options(),
    );
    assert_eq!((report.images, report.skipped_images), (2, 3));
    assert!(
        doc.contains(
            "<image x=\"10\" y=\"20\" width=\"40\" height=\"10\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,iVBORw0KGgpyZXN0\"/>"
        ),
        "{doc}"
    );
    assert!(doc.contains("href=\"data:image/jpeg;base64,/9j/4AEC\""), "{doc}");
    assert_eq!(count(&doc, "<image "), 2);
}
