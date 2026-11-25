// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The printed text of a value of the domain (`ew_derive::text`): one form each, wherever it prints.

use ew_schema::{ColourSpec, Csa};

/// A gauge from 1/0 up prints as the wire trade writes it, `AWG2/0`; a smaller wire as its number.
#[test]
fn a_heavy_gauge_prints_with_its_zeros() {
    assert_eq!(ew_derive::csa_text(&Csa::Awg(-1)).as_deref(), Some("AWG2/0"));
    assert_eq!(ew_derive::csa_text(&Csa::Awg(0)).as_deref(), Some("AWG1/0"));
    assert_eq!(ew_derive::csa_text(&Csa::Awg(18)).as_deref(), Some("AWG18"));
}

/// A colour prints its codes as written, a code this build does not know as `?`, in every place that prints it.
#[test]
fn an_unknown_colour_code_prints_one_word() {
    let colour = |v: serde_json::Value| -> String {
        ew_derive::colour_text(&serde_json::from_value::<ColourSpec>(v).expect("a colour"))
    };
    assert_eq!(colour(serde_json::json!(["GN", "YE"])), "GNYE");
    assert_eq!(
        colour(serde_json::json!([{"foreign": "ROSA"}, {"newer": 1}])),
        "ROSA?"
    );
}
