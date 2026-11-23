// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writes the key list of the shell's text folder (`ew_l10n::build::generate`), so that `key!` and `count_key!` check a
//! key when the code is built.

fn main() {
    ew_l10n::build::generate("locales");
}
