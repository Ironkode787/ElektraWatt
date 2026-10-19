// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the renderer tests: sheets, bundled fonts, text runs and pixel probes.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use ew_dl::{Brush, FontKey, ShapedRun};
use ew_render_cpu::{FontBytes, RgbaImage, SheetSize};
use ew_units::{Nm, Pt2};
use sha2::{Digest, Sha256};

/// A4 landscape, 297 mm × 210 mm.
pub fn a4() -> SheetSize {
    SheetSize {
        width: Nm::from_mm(297),
        height: Nm::from_mm(210),
    }
}

/// A point in whole millimetres.
pub fn mm(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm::from_mm(x), Nm::from_mm(y))
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn font_file(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "assets", "fonts", name]
        .iter()
        .collect();
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The bundled Noto Sans files keyed by their content hash (the key `ew-text` puts into runs).
pub fn bundled_fonts() -> BTreeMap<FontKey, FontBytes> {
    let mut map = BTreeMap::new();
    for (name, want) in [
        ("NotoSans-Regular.ttf", ew_text::NOTO_SANS_REGULAR_SHA256),
        ("NotoSans-Bold.ttf", ew_text::NOTO_SANS_BOLD_SHA256),
    ] {
        let bytes = font_file(name);
        let sha = sha256(&bytes);
        assert_eq!(sha, want, "{name} is not the pinned bundled file");
        map.insert(
            FontKey {
                sha256: sha,
                face_index: 0,
            },
            FontBytes {
                data: Arc::from(bytes),
                index: 0,
            },
        );
    }
    map
}

/// `text` shaped in the `text.normal` style, placed at `origin` in black.
pub fn text_run(text: &str, origin: Pt2) -> ShapedRun {
    let style = ew_text::builtin().resolve("text.normal").expect("builtin style");
    let shaped = ew_text::bundled().shape(text, &style, None);
    let mut run = (*shaped.run).clone();
    run.origin = origin;
    run.brush = Brush::BLACK;
    run
}

pub fn pixel(img: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    let i = (y * img.width + x) as usize * 4;
    [
        img.pixels[i],
        img.pixels[i + 1],
        img.pixels[i + 2],
        img.pixels[i + 3],
    ]
}

/// Dark means every colour channel below 128.
pub fn is_dark(p: [u8; 4]) -> bool {
    p[0] < 128 && p[1] < 128 && p[2] < 128
}

/// A deterministic linear congruential generator.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// A value in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}
