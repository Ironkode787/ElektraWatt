// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import qet --out a/b.ew` where the folder `a` does not exist makes it and writes the project there (FT-1868):
//! before, the import ran and then wrote nothing.

use crate::support::{Tmp, args, stderr};

const PROJECT: &str = r#"<project version="0.100" title="T"><diagram order="1" folio="1" title="P"><elements/><conductors/></diagram></project>"#;

#[test]
fn an_import_into_a_missing_folder_makes_it_and_writes_the_project() {
    let t = Tmp::new("ew-import-missing-folder");
    let qet = t.path("t.qet");
    std::fs::write(&qet, PROJECT).unwrap();
    let out = t.path("a").join("b.ew");
    let o = t.ew(&args([&"import", &"qet", &qet, &"--out", &out]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(out.join("manifest.json").is_file(), "the project is written");
}
