// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Portable draft and EPLAN input boundaries: accepted drafts can validate and preview without panicking.
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if let Ok(draft) = ew_api::symbols::SymbolDraft::parse(data) {
        let _ = draft.validate();
        for variant in 0..8 {
            let _ = draft.preview(variant);
        }
    }
    for name in ["sample.edz", "sample.ems", "sample.xml"] {
        if let Ok(drafts) = ew_api::symbols::import_eplan(data, name, "fuzz.symbol") {
            for draft in drafts {
                let _ = draft.validate();
                let _ = draft.preview(0);
            }
        }
    }
});
