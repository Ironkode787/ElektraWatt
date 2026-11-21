// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The raster pictures of a DXF page (doc 09 §4.9): a PNG or JPEG image item is an `IMAGE` that names a file the export
//! writes beside the pages (`picture-<12 hex digits of its SHA-256>.png` or `.jpg`), with its `IMAGEDEF` in the image
//! dictionary (`ACAD_IMAGE_DICT`), a reactor (`IMAGEDEF_REACTOR`) per `IMAGE` that ties the two, the raster variables
//! (`ACAD_IMAGE_VARS`) and the classes of these objects, as AutoCAD writes them, so that a converter carries them into a
//! DWG unchanged. An SVG picture never reaches this module: the export job has made it paths.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use ew_dl::{ImageData, ImageFormat};

use super::DxfPicture;

/// One picture file of the set.
#[derive(Clone, Debug)]
pub(super) struct Picture {
    /// Its file name, `picture-<hex>.<ext>`.
    pub name: String,
    /// Its width and height in pixels.
    pub pixels: (u16, u16),
    /// The file.
    pub bytes: Arc<[u8]>,
    /// Whether it is a PNG (which may be transparent).
    pub png: bool,
}

/// The pictures of the set by their SHA-256: `None` for bytes the renderers' decoder does not take (not written; the
/// export job reports them as page findings).
#[derive(Default)]
pub(super) struct Pictures(BTreeMap<[u8; 32], Option<Picture>>);

impl Pictures {
    /// The picture of `image`, decoded once for the set; `None` for an SVG, empty bytes or bytes that do not decode.
    pub(super) fn get(&mut self, image: &ImageData) -> Option<Picture> {
        if image.bytes.is_empty() || image.format == ImageFormat::Svg {
            return None;
        }
        self.0
            .entry(image.sha256)
            .or_insert_with(|| {
                let decoded = ew_render_cpu::decode_image(image.format, &image.bytes)?;
                let hex: String = image.sha256[..6].iter().map(|b| format!("{b:02x}")).collect();
                let png = image.format == ImageFormat::Png;
                Some(Picture {
                    name: format!("picture-{hex}.{}", if png { super::PNG } else { super::JPG }),
                    pixels: (decoded.width, decoded.height),
                    bytes: Arc::clone(&image.bytes),
                    png,
                })
            })
            .clone()
    }

    /// The files the set's pages name, in the order of their names.
    pub(super) fn files(self) -> Vec<DxfPicture> {
        let mut out: Vec<DxfPicture> = self
            .0
            .into_values()
            .flatten()
            .map(|p| DxfPicture {
                name: p.name,
                bytes: p.bytes,
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

/// The pictures one page draws, with the handles of their objects in its file.
#[derive(Default)]
pub(super) struct PagePictures {
    /// Each picture with the size of a pixel where it is first drawn (mm), in first-use order.
    pub used: Vec<(Picture, (f64, f64))>,
    /// The handle of each picture's `IMAGEDEF`, in the order of [`PagePictures::used`].
    pub defs: Vec<String>,
    /// Each `IMAGE` written: its picture, its own handle and its reactor's.
    pub images: Vec<(usize, String, String)>,
}

impl PagePictures {
    /// The index of `picture` on this page, drawn with pixels of `pixel` mm; added at its first use.
    pub(super) fn index(&mut self, picture: Picture, pixel: (f64, f64)) -> usize {
        match self.used.iter().position(|(p, _)| p.name == picture.name) {
            Some(i) => i,
            None => {
                self.used.push((picture, pixel));
                self.used.len() - 1
            }
        }
    }

    /// The groups of an `IMAGE` of picture `k` after its corner and its pixel vectors: its definition, its display
    /// (shown, also turned, and transparent for a PNG), its reactor and its clip boundary, the whole picture.
    pub(super) fn image_tail(&self, k: usize, reactor: &str) -> String {
        let (p, _) = &self.used[k];
        let flags = if p.png { 1 | 2 | 8 } else { 1 | 2 };
        let (w, h) = p.pixels;
        format!(
            "340\n{}\n 70\n{flags}\n280\n0\n281\n50\n282\n50\n283\n0\n360\n{reactor}\n 71\n1\n 91\n2\n 14\n-0.5\n 24\n-0.5\n 14\n{}\n 24\n{}\n290\n0\n",
            self.defs[k],
            f64::from(w) - 0.5,
            f64::from(h) - 0.5,
        )
    }

    /// The CLASSES section's records of the picture objects with their instance counts; empty without a picture.
    pub(super) fn classes(&self) -> String {
        if self.used.is_empty() {
            return String::new();
        }
        let mut s = String::new();
        for (name, class, flags, count, entity) in [
            ("IMAGE", "AcDbRasterImage", 2175, self.images.len(), 1),
            ("IMAGEDEF", "AcDbRasterImageDef", 0, self.used.len(), 0),
            (
                "IMAGEDEF_REACTOR",
                "AcDbRasterImageDefReactor",
                1,
                self.images.len(),
                0,
            ),
            ("RASTERVARIABLES", "AcDbRasterVariables", 0, 1, 0),
        ] {
            let _ = write!(
                s,
                "  0\nCLASS\n  1\n{name}\n  2\n{class}\n  3\nISM\n 90\n{flags}\n 91\n{count}\n280\n0\n281\n{entity}\n"
            );
        }
        s
    }

    /// The root dictionary's entries of the pictures: the image dictionary and the raster variables.
    pub(super) fn root_entries(&self, dict: &str, vars: &str) -> String {
        if self.used.is_empty() {
            return String::new();
        }
        format!("  3\nACAD_IMAGE_DICT\n350\n{dict}\n  3\nACAD_IMAGE_VARS\n350\n{vars}\n")
    }

    /// The objects of the pictures: the image dictionary, each `IMAGEDEF` with its reactors, each reactor, and the
    /// raster variables (no frame, high quality, no unit).
    pub(super) fn objects(&self, root: &str, dict: &str, vars: &str) -> String {
        if self.used.is_empty() {
            return String::new();
        }
        let mut s = format!("  0\nDICTIONARY\n  5\n{dict}\n330\n{root}\n100\nAcDbDictionary\n281\n1\n");
        for ((p, _), def) in self.used.iter().zip(&self.defs) {
            let _ = write!(
                s,
                "  3\n{}\n350\n{def}\n",
                p.name.rsplit_once('.').map_or(p.name.as_str(), |(n, _)| n)
            );
        }
        for (k, ((p, pixel), def)) in self.used.iter().zip(&self.defs).enumerate() {
            let _ = write!(
                s,
                "  0\nIMAGEDEF\n  5\n{def}\n102\n{{ACAD_REACTORS\n330\n{dict}\n"
            );
            for (_, _, reactor) in self.images.iter().filter(|(i, _, _)| *i == k) {
                let _ = write!(s, "330\n{reactor}\n");
            }
            let _ = write!(
                s,
                "102\n}}\n330\n{dict}\n100\nAcDbRasterImageDef\n 90\n0\n  1\n{}\n 10\n{}\n 20\n{}\n 11\n{}\n 21\n{}\n280\n1\n281\n0\n",
                p.name,
                p.pixels.0,
                p.pixels.1,
                super::real(pixel.0),
                super::real(pixel.1),
            );
        }
        for (_, image, reactor) in &self.images {
            let _ = write!(
                s,
                "  0\nIMAGEDEF_REACTOR\n  5\n{reactor}\n330\n{image}\n100\nAcDbRasterImageDefReactor\n 90\n2\n330\n{image}\n"
            );
        }
        let _ = write!(
            s,
            "  0\nRASTERVARIABLES\n  5\n{vars}\n102\n{{ACAD_REACTORS\n330\n{root}\n102\n}}\n330\n{root}\n100\nAcDbRasterVariables\n 90\n0\n 70\n0\n 71\n1\n 72\n0\n"
        );
        s
    }
}
