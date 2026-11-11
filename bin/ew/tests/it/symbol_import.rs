// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew symbol import` refuses a hostile EPLAN input in words with exit 2 (SM25#1, SM25#3) and reads a `.pxf` file
//! as PXF (SM25#4).

use crate::support::{EwTmp, Tmp, args, stderr, stdout};

#[test]
fn hostile_eplan_input_is_refused_in_words_and_pxf_is_read() {
    let t = Tmp::new("ew-tables");
    let deep = t.0.join("deep.xml");
    std::fs::write(
        &deep,
        format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000)),
    )
    .unwrap();
    let out = t.ew(&args([&"symbol", &"import", &deep]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        stderr(&out),
        "error: The EPLAN input exceeds the supported size or complexity.\n"
    );
    let pxf = t.0.join("macro.pxf");
    std::fs::write(
        &pxf,
        br#"<EplanPxfRoot PxfVersion="1.23" Name="M"><MacroVariant><O31 A531="0/0" A532="4/0"/></MacroVariant></EplanPxfRoot>"#,
    )
    .unwrap();
    let out = t.ew(&args([&"symbol", &"import", &pxf]));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let drafts: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(drafts.as_array().map(Vec::len), Some(1));
}
