// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1925: the sample verified against the PDF it writes deviates nowhere, its parts lists' wrapped descriptions
//! included (doc 14 §4.15).

use std::sync::Arc;

use crate::common;

use ew_api::migration::verify_migration;
use ew_api::output::ExportOptions;

#[test]
fn the_sample_matches_its_own_pdf() {
    let session = common::sample();
    let pdf: Arc<[u8]> = session
        .export_pdf(&ExportOptions::default())
        .expect("the export")
        .bytes
        .into();
    let report = verify_migration(&session, pdf, &[]).expect("a verification");
    assert!(report.is_clean(), "{:#?}", report.deviations);
}
