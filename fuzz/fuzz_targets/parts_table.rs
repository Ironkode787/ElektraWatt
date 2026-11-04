// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `parts_table` importer target (WP-S3.09 D15): see `ew_fuzz::importers::parts_table`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ew_fuzz::importers::parts_table(data);
});
