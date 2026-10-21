// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF (outline, links, named destinations, page labels, PDF/A-2b), SVG, DXF and PNG export.
//!
//! So far (WP-S3.04): the PDF writer [`pdf_bytes`] / [`pdf`] over sheet display lists (doc 09 §4.2, §4.3;
//! ADR-0015 decision 5), and PNG through the CPU renderer, re-exported as [`ew_render_cpu`]. Cycle 14: the
//! SVG writer [`svg`] over the same display lists (doc 09 §4.4, FR-OUT-013) and the parallel page phase
//! [`par_map`] with results by page index (FR-OUT-019, AC-14).

#![forbid(unsafe_code)]

mod parallel;
mod pdf;
mod svg;

pub use ew_render_cpu;
pub use parallel::{Parallelism, par_map};
pub use pdf::{
    Conformance, OutlineEntry, PartialLinks, PdfDate, PdfError, PdfOptions, PdfOutput, PdfPage, PdfReport,
    outline_entries, pdf, pdf_bytes,
};
pub use svg::{SvgError, SvgFace, SvgFile, SvgOptions, SvgOutput, SvgReport, svg, svg_file_name, svg_id};
