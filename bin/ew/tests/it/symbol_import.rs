// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew symbol import` refuses a hostile EPLAN input in words with exit 2 (SM25#1, SM25#3) and reads a `.pxf` file
//! as PXF (SM25#4).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The path of the `ew` binary under test (nextest's run-time path, else the compile-time one).
// The program boundary of a test: the path only chooses which binary runs; no compared result depends on it.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_ew")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ew"))) // bin-exe-fallback
}

fn import(dir: &Path, file: &Path) -> Output {
    Command::new(bin_exe())
        .args(["symbol", "import"])
        .arg(file)
        .current_dir(dir)
        .env("EW_DATA_DIR", dir.join("data"))
        .env("EW_CACHE_DIR", dir.join("cache"))
        .output()
        .unwrap()
}

#[test]
fn hostile_eplan_input_is_refused_in_words_and_pxf_is_read() {
    let dir = ew_testdir::fresh_dir("ew-symbol-import");
    let deep = dir.join("deep.xml");
    std::fs::write(
        &deep,
        format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000)),
    )
    .unwrap();
    let out = import(&dir, &deep);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "error: The EPLAN input exceeds the supported size or complexity.\n"
    );
    let pxf = dir.join("macro.pxf");
    std::fs::write(
        &pxf,
        br#"<EplanPxfRoot PxfVersion="1.23" Name="M"><MacroVariant><O31 A531="0/0" A532="4/0"/></MacroVariant></EplanPxfRoot>"#,
    )
    .unwrap();
    let out = import(&dir, &pxf);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let drafts: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(drafts.as_array().map(Vec::len), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}
