// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fonts of the SVG pages (SV-1 in miniature): each page's `<style>` declares
//! exactly the faces its `<text>` elements name, one subset per face per page, and every character of the page's
//! texts maps through the embedded subset's `cmap` to the face's glyph with its outline (read back with
//! `skrifa`); a face without usable bytes is named only and reported.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_dl::{Destination, DisplayList, FontKey, Item};
use ew_export::{Parallelism, SvgFace, svg};
use ew_text::{Family, TextStyle, Weight, bundled};
use ew_units::{Nm, Pt2};
use uuid::Uuid;

use crate::font_subset::{glyph_bytes, glyph_of, outline_commands};
use crate::svg::{faces, options};

fn style(family: Family, weight: Weight) -> TextStyle {
    TextStyle {
        family,
        weight,
        ..TextStyle::regular(Nm::from_mm(3))
    }
}

fn text(s: &str, style: &TextStyle, y: i64) -> Item {
    let shaped = bundled().shape(s, style, None);
    assert_eq!(shaped.faces.len(), 1, "{s}: one face draws it");
    let mut run = (*shaped.run).clone();
    run.origin = Pt2::new(Nm::from_mm(20), Nm::from_mm(y));
    Item::glyphs(Arc::new(run))
}

fn page(n: u128, items: Vec<Item>) -> ew_export::PdfPage {
    ew_export::PdfPage {
        destination: Destination::Page(Uuid::from_u128(n)),
        label: n.to_string(),
        title: n.to_string(),
        section: None,
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList { items },
        placement: None,
    }
}

/// Base64 (RFC 4648, padded) back to bytes.
fn unbase64(s: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {c}"),
    };
    let mut out = Vec::new();
    for chunk in s.as_bytes().chunks(4) {
        let pad = chunk.iter().filter(|&&c| c == b'=').count();
        let n = chunk
            .iter()
            .map(|&c| if c == b'=' { 0 } else { u32::from(value(c)) })
            .fold(0u32, |n, v| (n << 6) | v);
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    out
}

/// One declared face of a page: family, weight, style and the decoded subset.
#[derive(Debug)]
struct Declared {
    family: String,
    weight: u16,
    italic: bool,
    subset: Vec<u8>,
}

/// The `@font-face` rules of a page's `<style>`, each written whole in the form D3 gives.
fn declared(doc: &str) -> Vec<Declared> {
    let style = &doc[doc.find("<style>").unwrap() + 7..doc.find("</style>").unwrap()];
    assert!(style.ends_with("text{white-space:pre}"), "{style:.80}");
    style
        .lines()
        .filter(|l| l.starts_with("@font-face"))
        .map(|l| {
            let rest = l.strip_prefix("@font-face { font-family: \"").unwrap();
            let (family, rest) = rest.split_once("\"; font-weight: ").unwrap();
            let (weight, rest) = rest.split_once("; font-style: ").unwrap();
            let (slope, rest) = rest.split_once("; src: url(data:font/ttf;base64,").unwrap();
            let data = rest.strip_suffix(") format(\"truetype\"); }").unwrap();
            Declared {
                family: family.to_owned(),
                weight: weight.parse().unwrap(),
                italic: match slope {
                    "normal" => false,
                    "italic" => true,
                    other => panic!("font-style {other}"),
                },
                subset: unbase64(data),
            }
        })
        .collect()
}

/// The `<text>` elements of a page: (family, weight, italic, the text unescaped).
fn texts(doc: &str) -> Vec<(String, u16, bool, String)> {
    doc.split("<text")
        .skip(1)
        .map(|el| {
            let open = &el[..el.find('>').unwrap()];
            let attr = |name: &str| {
                let key = format!(" {name}=\"");
                open.find(&key).map(|i| {
                    let v = &open[i + key.len()..];
                    v[..v.find('"').unwrap()].to_owned()
                })
            };
            let family = attr("font-family").unwrap();
            let family = family.strip_suffix(", sans-serif").unwrap().to_owned();
            let weight = attr("font-weight").map_or(400, |w| w.parse().unwrap());
            let italic = attr("font-style").is_some_and(|s| s == "italic");
            let body = &el[el.find('>').unwrap() + 1..el.find("</text>").unwrap()];
            let body = body
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&");
            (family, weight, italic, body)
        })
        .collect()
}

/// The bundled face named by (family, weight, italic).
fn face_bytes(family: &str, weight: u16, italic: bool) -> &'static [u8] {
    let f = bundled()
        .faces()
        .find(|f| f.family_name == family && f.weight.value() == weight && f.italic == italic)
        .unwrap();
    bundled().face_data(f.key).unwrap()
}

/// SV-1 for one page: the declared faces are exactly the named ones, once each, and every character of every
/// `<text>` maps through the declared subset to the face's glyph with the face's outline bytes (an outline that
/// draws, unless the face's own glyph is empty, as a space's is).
fn check_page(doc: &str) -> BTreeSet<(String, u16, bool)> {
    let declared = declared(doc);
    let texts = texts(doc);
    let named: BTreeSet<(String, u16, bool)> = texts.iter().map(|t| (t.0.clone(), t.1, t.2)).collect();
    let decl: Vec<(String, u16, bool)> = declared
        .iter()
        .map(|d| (d.family.clone(), d.weight, d.italic))
        .collect();
    assert_eq!(
        decl.iter().cloned().collect::<BTreeSet<_>>(),
        named,
        "exactly the named faces"
    );
    assert_eq!(decl.len(), named.len(), "each face once");
    for (family, weight, italic, body) in &texts {
        let d = declared
            .iter()
            .find(|d| (&d.family, d.weight, d.italic) == (family, *weight, *italic))
            .unwrap();
        let face = face_bytes(family, *weight, *italic);
        let (ours, theirs) = (glyph_bytes(&d.subset), glyph_bytes(face));
        for c in body.chars() {
            let g = glyph_of(&d.subset, c).unwrap_or_else(|| panic!("{c} maps in the subset"));
            assert_eq!(Some(g), glyph_of(face, c), "{c}: the face's glyph ID");
            assert_eq!(
                ours[usize::from(g)],
                theirs[usize::from(g)],
                "{c}: the face's outline bytes"
            );
            if !theirs[usize::from(g)].is_empty() {
                assert!(outline_commands(&d.subset, g) > 0, "{c} draws");
            }
        }
    }
    named
}

/// SV-1 in miniature: a page with Regular, Bold and the symbol face declares those three; a page with Regular
/// only declares one, whose subset lacks the other page's glyphs; a page without text declares none.
#[test]
fn each_page_embeds_exactly_the_faces_its_texts_name() {
    let regular = style(Family::Default, Weight::Regular);
    let bold = style(Family::Default, Weight::Bold);
    let symbols = style(Family::Symbols, Weight::Regular);
    let pages = [
        page(
            1,
            vec![
                text("Hello Wörld <&>", &regular, 20),
                text("Bold 24 V", &bold, 30),
                text("∑∫", &symbols, 40),
            ],
        ),
        page(2, vec![text("xyz", &regular, 20)]),
        page(3, vec![]),
    ];
    let out = svg(&pages, &options(), None, Parallelism::Serial).unwrap();
    assert!(out.report.named_fonts.is_empty(), "{:?}", out.report);
    let first = check_page(&out.files[0].text);
    let families: BTreeSet<&str> = first.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(families, BTreeSet::from(["Noto Sans", "Noto Sans Math"]));
    assert_eq!(first.len(), 3);
    assert_eq!(check_page(&out.files[1].text).len(), 1);
    assert_eq!(check_page(&out.files[2].text).len(), 0);
    assert!(
        out.files[2]
            .text
            .contains("<style>text{white-space:pre}</style>\n"),
        "a page without text has no @font-face"
    );
    // The pages are independent: page 2's subset holds its own glyphs only.
    let page2 = &declared(&out.files[1].text)[0].subset;
    let h = glyph_of(page2, 'H').unwrap();
    assert!(
        glyph_bytes(page2)[usize::from(h)].is_empty(),
        "page 1's H is not on page 2"
    );
    let x = glyph_of(page2, 'x').unwrap();
    assert!(!glyph_bytes(page2)[usize::from(x)].is_empty());
    // The thread count changes no byte.
    for n in [2, 3] {
        let p = svg(&pages, &options(), None, Parallelism::threads(n)).unwrap();
        assert_eq!(p, out, "{n} threads");
    }
}

/// The face rule as D3 writes it, whole up to the data.
#[test]
fn the_font_face_rule_is_written_in_the_form_of_d3() {
    let out = svg(
        &[page(
            1,
            vec![text("A", &style(Family::Default, Weight::Bold), 20)],
        )],
        &options(),
        None,
        Parallelism::Serial,
    )
    .unwrap();
    let doc = &out.files[0].text;
    let at = doc.find("<style>").unwrap();
    let rule = &doc[at..doc[at..].find("base64,").unwrap() + at + 7];
    assert_eq!(
        rule,
        "<style>@font-face { font-family: \"Noto Sans\"; font-weight: 700; font-style: normal; src: url(data:font/ttf;base64,"
    );
    assert!(doc.contains(") format(\"truetype\"); }\ntext{white-space:pre}</style>\n<text"));
}

/// A face without bytes, or with bytes the subsetter refuses, is named only: its text is written, no
/// `@font-face` declares it, and the report lists it once.
#[test]
fn a_face_without_usable_bytes_is_named_and_reported() {
    let regular = style(Family::Default, Weight::Regular);
    let key: FontKey = bundled().shape("A", &regular, None).run.font;
    let pages = [
        page(1, vec![text("Named", &regular, 20)]),
        page(2, vec![text("Again", &regular, 20)]),
    ];
    // FT-1046 (coverage, ID-1138: "FT-1046's two cases are coverage tests that pass before the change"): a face of
    // a collection (`ttcf`) is refused by the subsetter like any other unusable bytes.
    let collection = Arc::<[u8]>::from(&b"ttcf\x00\x02\x00\x00\x00\x00\x00\x01\x00\x00\x00\x10"[..]);
    for data in [
        None,
        Some(Arc::<[u8]>::from(&b"not a font"[..])),
        Some(collection),
    ] {
        let mut opts = options();
        let face: &mut SvgFace = opts.faces.get_mut(&key).unwrap();
        face.data = data.clone();
        let out = svg(&pages, &opts, None, Parallelism::Serial).unwrap();
        assert_eq!(out.report.named_fonts, vec![key], "{data:?}");
        assert_eq!(out.report.texts, 2);
        for f in &out.files {
            assert!(
                f.text.contains("<style>text{white-space:pre}</style>\n"),
                "{}",
                f.name
            );
            assert!(f.text.contains("font-family=\"Noto Sans, sans-serif\""));
        }
    }
    // FT-1046 (coverage): a run glyph ID above 65535 is not a glyph of a TrueType face, so the subsetter refuses
    // the face, which is named only, though its bytes are the bundled face's.
    let mut run = (*bundled().shape("Named", &regular, None).run).clone();
    run.origin = Pt2::new(Nm::from_mm(20), Nm::from_mm(20));
    run.glyphs[0].id = 70_000;
    let out = svg(
        &[page(1, vec![Item::glyphs(Arc::new(run))])],
        &options(),
        None,
        Parallelism::Serial,
    )
    .unwrap();
    assert_eq!(out.report.named_fonts, vec![key]);
    assert_eq!(out.report.texts, 1);
    assert!(declared(&out.files[0].text).is_empty());
    assert!(
        out.files[0]
            .text
            .contains("<style>text{white-space:pre}</style>\n")
    );
    let faces: BTreeMap<FontKey, SvgFace> = faces();
    assert!(faces.values().all(|f| f.data.is_some()));
}

/// "the subset's characters are those written after escaping". A run whose text holds a control
/// character is written with U+FFFD in its place, and the page's subset holds U+FFFD's outline, so a browser that
/// maps the characters itself draws what the page shows.
#[test]
fn a_control_character_is_written_as_u_fffd_and_its_outline_is_embedded() {
    let regular = style(Family::Default, Weight::Regular);
    let face = face_bytes("Noto Sans", 400, false);
    assert!(
        glyph_of(face, '\u{FFFD}').is_some(),
        "the bundled face maps U+FFFD"
    );
    let mut run = (*bundled().shape("AxB", &regular, None).run).clone();
    run.origin = Pt2::new(Nm::from_mm(20), Nm::from_mm(20));
    run.text = "A\u{1}B".to_owned();
    let out = svg(
        &[page(1, vec![Item::glyphs(Arc::new(run))])],
        &options(),
        None,
        Parallelism::Serial,
    )
    .unwrap();
    assert!(out.report.named_fonts.is_empty(), "{:?}", out.report);
    let doc = &out.files[0].text;
    let bodies: Vec<String> = texts(doc).into_iter().map(|t| t.3).collect();
    assert_eq!(bodies, ["A\u{FFFD}B"]);
    // Every written character maps through the subset to the face's glyph and outline bytes, U+FFFD's included.
    check_page(doc);
}
