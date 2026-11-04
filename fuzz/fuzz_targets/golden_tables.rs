// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `golden_tables` importer target (WP-S3.09 D15): see `ew_fuzz::importers::golden_tables`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ew_fuzz::importers::golden_tables(data);
});
