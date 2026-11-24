// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the key table of this crate's text folder (`ew_l10n::build::generate`), which `text!` and `key!` check when
//! the code is built.

fn main() {
    ew_l10n::build::generate("texts");
}
