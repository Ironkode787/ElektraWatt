// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PDF (outline, links, named destinations, page labels, PDF/A-2b), SVG, DXF and PNG export.
//!
//! So far (WP-S3.04): the PDF writer [`pdf_bytes`] / [`pdf`] over sheet display lists (doc 09 §4.2, §4.3;
//! ADR-0015 decision 5), and PNG through the CPU renderer, re-exported as [`ew_render_cpu`].

#![forbid(unsafe_code)]

mod pdf;

pub use ew_render_cpu;
pub use pdf::{
    Conformance, OutlineEntry, PartialLinks, PdfDate, PdfError, PdfOptions, PdfOutput, PdfPage, PdfReport,
    outline_entries, pdf, pdf_bytes,
};
