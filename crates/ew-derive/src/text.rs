// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a value of the domain is printed as a string that no language changes (doc 04 §3.4): one function per value.
//! Where two places print a value differently, both forms stand here side by side under names that say the
//! difference, and each caller keeps the form it had.

use ew_schema::{ColourSpec, Csa};

/// The printed text of a cross-section (`2.5`, `AWG18`, `250 kcmil`); `None` for a value of a newer format.
pub fn csa_text(c: &Csa) -> Option<String> {
    match c {
        Csa::Mm2(d) => Some(d.to_string()),
        Csa::Awg(n) => Some(format!("AWG{n}")),
        Csa::Kcmil(d) => Some(format!("{d} kcmil")),
        Csa::Unknown(_) => None,
    }
}

/// The printed colour code of a colour specification: its codes in their file spelling, concatenated (`BU`,
/// `GNYE`, `DBU`); `None` when a code has no text form (a value of a newer format).
pub fn colour_text(c: &ColourSpec) -> Option<String> {
    let mut out = String::new();
    for code in c.codes() {
        let v = serde_json::to_value(code).ok()?;
        out.push_str(v.as_str()?);
    }
    Some(out)
}

