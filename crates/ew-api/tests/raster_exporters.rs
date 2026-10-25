// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-436: the real-exporter raster corpus through the asset sniffer.

use std::path::Path;

use ew_api::AssetFormat;

/// The real-exporter raster corpus (FT-436): file name, format and the size of the corpus table.
const CORPUS: &[(&str, AssetFormat, (u32, u32))] = &[
    ("jpeg-progressive-gimp-icc.jpg", AssetFormat::Jpeg, (945, 945)),
    ("jpeg-progressive-photoshop.jpg", AssetFormat::Jpeg, (600, 362)),
    ("jpeg-illustrator-export.jpg", AssetFormat::Jpeg, (2560, 1707)),
    ("png-rgba-inkscape-export.png", AssetFormat::Png, (10, 10)),
    ("png-rgba-translucent.png", AssetFormat::Png, (803, 770)),
    ("png-rgba-gama.png", AssetFormat::Png, (1100, 550)),
    ("png-palette-trns.png", AssetFormat::Png, (1056, 107)),
    ("png-grey-gama.png", AssetFormat::Png, (257, 257)),
];

#[test]
fn every_raster_file_passes_the_asset_sniff() {
    let dir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ew-export/tests/fixtures/raster-exporters"
    ));
    let mut mismatches = Vec::new();
    for (name, format, size) in CORPUS {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        match ew_api::sniff_asset(&bytes) {
            Ok((found, Some((w, h)))) if found == *format && (w, h) == *size => {}
            other => mismatches.push(format!("{name}: sniffed {other:?}, ruled {format:?} {size:?}")),
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} corpus file(s) did not sniff as ruled:\n{}",
        mismatches.len(),
        CORPUS.len(),
        mismatches.join("\n")
    );
}
