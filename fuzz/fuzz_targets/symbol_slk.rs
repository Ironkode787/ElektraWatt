// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `symbol_slk` target (M25-p2#6): see `ew_fuzz::symbols::slk`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = ew_fuzz::symbols::slk(data);
});
