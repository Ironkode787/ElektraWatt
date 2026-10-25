// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-436: real exporters' raster files through `decode_image` and the PDF writer.

// EW_RASTER_PDF_DIR names a folder that the test writes each PDF to for veraPDF; it never changes the compared bytes.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_dl::{Destination, DisplayList, FontKey, Item};
use ew_export::ew_render_cpu::FontBytes;
use ew_export::{PdfDate, PdfOptions, PdfPage};
use ew_units::{Nm, Pt2, Rect};
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn fonts() -> BTreeMap<FontKey, FontBytes> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut map = BTreeMap::new();
    for name in ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf"] {
        let bytes = std::fs::read(root.join(name)).unwrap();
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        let key = FontKey {
            sha256,
            face_index: 0,
        };
        map.insert(
            key,
            FontBytes {
                data: Arc::from(bytes),
                index: 0,
            },
        );
    }
    map
}

fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

fn page(n: u128, label: &str, section: Option<&str>, items: Vec<Item>) -> PdfPage {
    PdfPage {
        destination: Destination::Page(Uuid::from_u128(n)),
        label: label.to_owned(),
        title: format!("{label} Page {label}"),
        section: section.map(str::to_owned),
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
        content: DisplayList { items },
    }
}

fn options() -> PdfOptions {
    PdfOptions {
        title: Some("Test project".into()),
        description: Some("drawing <drawing id>, revision V2.0".into()),
        keywords: vec!["ElektraWatt".into()],
        language: Some("en".into()),
        creator: Some("ElektraWatt 0.0.0".into()),
        document_id: "test-export-input".into(),
        ..PdfOptions::new(PdfDate::ymd(2026, 3, 14))
    }
}

fn count(hay: &[u8], needle: &str) -> usize {
    hay.windows(needle.len())
        .filter(|w| *w == needle.as_bytes())
        .count()
}

fn contains(hay: &[u8], needle: &str) -> bool {
    count(hay, needle) > 0
}

fn image(sha: u8, format: ew_dl::ImageFormat, bytes: &[u8], from: Pt2, to: Pt2) -> Item {
    Item::image(
        Arc::new(ew_dl::ImageData {
            sha256: [sha; 32],
            format,
            bytes: Arc::from(bytes),
        }),
        Rect::from_points(from, to),
    )
}

/// What the lead ruled for one corpus file.
struct Ruled {
    /// `Png` or `Jpeg`.
    format: ew_dl::ImageFormat,
    /// The decoded size, or `None` when `decode_image` must refuse the file.
    size: Option<(u16, u16)>,
    /// What `jpeg_is_cmyk` says (false for a PNG).
    cmyk: bool,
}

/// The real-exporter raster corpus (FT-436), in the table order of `SOURCES.md`.
const CORPUS: &[(&str, Ruled)] = &[
    (
        "jpeg-progressive-gimp-icc.jpg",
        Ruled {
            format: ew_dl::ImageFormat::Jpeg,
            size: Some((945, 945)),
            cmyk: false,
        },
    ),
    (
        "jpeg-progressive-photoshop.jpg",
        Ruled {
            format: ew_dl::ImageFormat::Jpeg,
            size: Some((600, 362)),
            cmyk: false,
        },
    ),
    (
        "jpeg-illustrator-export.jpg",
        Ruled {
            format: ew_dl::ImageFormat::Jpeg,
            size: Some((2560, 1707)),
            cmyk: false,
        },
    ),
    (
        "png-rgba-inkscape-export.png",
        Ruled {
            format: ew_dl::ImageFormat::Png,
            size: Some((10, 10)),
            cmyk: false,
        },
    ),
    (
        "png-rgba-translucent.png",
        Ruled {
            format: ew_dl::ImageFormat::Png,
            size: Some((803, 770)),
            cmyk: false,
        },
    ),
    (
        "png-rgba-gama.png",
        Ruled {
            format: ew_dl::ImageFormat::Png,
            size: Some((1100, 550)),
            cmyk: false,
        },
    ),
    (
        "png-palette-trns.png",
        Ruled {
            format: ew_dl::ImageFormat::Png,
            size: Some((1056, 107)),
            cmyk: false,
        },
    ),
    (
        "png-grey-gama.png",
        Ruled {
            format: ew_dl::ImageFormat::Png,
            size: Some((257, 257)),
            cmyk: false,
        },
    ),
];

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raster-exporters")
}

#[test]
fn every_raster_file_decodes_as_ruled() {
    let dir = corpus_dir();
    let mut mismatches = Vec::new();
    for (name, ruled) in CORPUS {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        match (
            ew_export::ew_render_cpu::decode_image(ruled.format, &bytes),
            ruled.size,
        ) {
            (Some(img), Some(size)) => {
                if (img.width, img.height) != size {
                    mismatches.push(format!(
                        "{name}: decoded as {}x{}, ruled {size:?}",
                        img.width, img.height
                    ));
                }
            }
            (None, None) => {}
            (None, Some(size)) => {
                mismatches.push(format!("{name}: decode_image refused it, ruled {size:?}"));
            }
            (Some(img), None) => mismatches.push(format!(
                "{name}: decoded as {}x{}, ruled a refusal",
                img.width, img.height
            )),
        }
        let cmyk = ew_export::ew_render_cpu::jpeg_is_cmyk(&bytes);
        if cmyk != ruled.cmyk {
            mismatches.push(format!("{name}: jpeg_is_cmyk is {cmyk}, ruled {}", ruled.cmyk));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} corpus file(s) differ from the ruling:\n{}",
        mismatches.len(),
        CORPUS.len(),
        mismatches.join("\n")
    );
}

#[test]
fn every_decodable_raster_file_prints_in_a_pdf_a() {
    let dir = corpus_dir();
    let out_dir = std::env::var_os("EW_RASTER_PDF_DIR").map(PathBuf::from);
    if let Some(out_dir) = &out_dir {
        std::fs::create_dir_all(out_dir).unwrap();
    }
    let mut mismatches = Vec::new();
    for (name, ruled) in CORPUS {
        if ruled.size.is_none() {
            continue;
        }
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let pages = [page(
            1,
            "1",
            None,
            vec![image(1, ruled.format, &bytes, mm(10, 10), mm(110, 80))],
        )];
        let out = match ew_export::pdf(&pages, &options(), &fonts(), None) {
            Ok(out) => out,
            Err(e) => {
                mismatches.push(format!("{name}: the PDF writer failed: {e}"));
                continue;
            }
        };
        if out.report.images != 1 {
            mismatches.push(format!("{name}: report.images is {}, ruled 1", out.report.images));
        }
        if out.report.skipped_images != 0 {
            mismatches.push(format!(
                "{name}: report.skipped_images is {}, ruled 0",
                out.report.skipped_images
            ));
        }
        let b = &out.bytes;
        if !contains(b, "/Subtype/Image") {
            mismatches.push(format!("{name}: the PDF has no /Subtype/Image"));
        }
        if ruled.format == ew_dl::ImageFormat::Jpeg {
            if ruled.cmyk {
                if contains(b, "/Filter/DCTDecode") {
                    mismatches.push(format!("{name}: the CMYK JPEG is embedded as DCT"));
                }
            } else {
                if !contains(b, "/Filter/DCTDecode") {
                    mismatches.push(format!("{name}: the JPEG is not embedded as DCT"));
                }
                if !b.windows(bytes.len()).any(|w| w == bytes.as_slice()) {
                    mismatches.push(format!("{name}: the JPEG's own bytes are not in the PDF"));
                }
            }
        }
        if let Some(out_dir) = &out_dir {
            let stem = Path::new(name).file_stem().unwrap().to_string_lossy();
            std::fs::write(out_dir.join(format!("{stem}.pdf")), b).unwrap();
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} corpus file(s) did not print as ruled:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn the_raster_corpus_holds_exactly_the_ruled_files() {
    fn walk(dir: &Path, base: &Path, files: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, base, files);
                continue;
            }
            let relative = path.strip_prefix(base).unwrap();
            let name = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if name != "SOURCES.md" {
                files.push(name);
            }
        }
    }
    let dir = corpus_dir();
    let mut found = Vec::new();
    walk(&dir, &dir, &mut found);
    found.sort();
    let mut ruled: Vec<String> = CORPUS.iter().map(|(name, _)| (*name).to_owned()).collect();
    ruled.sort();
    assert_eq!(
        found, ruled,
        "the corpus folder holds a different file set than CORPUS"
    );
}

#[test]
fn sources_lists_every_raster_file() {
    let sources = std::fs::read_to_string(corpus_dir().join("SOURCES.md")).unwrap();
    let mut missing = Vec::new();
    for (name, _) in CORPUS {
        if !sources.contains(&format!("`{name}`")) {
            missing.push(*name);
        }
    }
    assert!(missing.is_empty(), "SOURCES.md does not name: {missing:?}");
}
