// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `.ewz` reader target (WP-S1.17 part 2): see `ew_fuzz::ewz_read`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ew_fuzz::ewz_read::check(data);
});
