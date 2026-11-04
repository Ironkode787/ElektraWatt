// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The WP-S6.04 target `ewlib_read` (ID-1354): see `ew_fuzz::ewlib_read`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ew_fuzz::ewlib_read::check(data);
});
