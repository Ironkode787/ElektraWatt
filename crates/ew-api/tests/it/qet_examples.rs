// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public QElectroTech example projects (SC-4, FR-IMP-006, doc 14 §4.15 and §12): every `.qet` of the folder
//! named by `EW_QET_EXAMPLES` imports without a structural error, the two projects the plan names lose no
//! connection, and importing a file twice gives the same batch, record for record.
//!
//! The examples are GPL files of the QElectroTech source mirror and are never committed; doc 14 §12 ("As built")
//! gives the pinned fetch into a scratch folder. The test is `#[ignore]`d (a deep-run test) and skips when the
//! variable is unset. Run it with
//! `EW_QET_EXAMPLES=<dir> cargo nextest run -p ew-api --test it -E 'test(/^qet_examples::/)' --run-ignored all`.

// EW_QET_EXAMPLES names the scratch folder of the examples; the result does not depend on the path.
#![allow(clippy::disallowed_methods)]

use crate::view;

use std::path::PathBuf;

use ew_api::import::{StarterLibrary, import_qet_batch};
use sha2::{Digest, Sha256};

/// The examples the plan says lose no connection: modern conductor ends (`photovoltaique`) and integer ends only
/// (`schema_indus`).
const LOSSLESS: [&str; 2] = ["photovoltaique.qet", "schema_indus.qet"];

/// How many of the examples may be refused (24 of them are corpus members; 22 must import, doc 14 §4.15).
const MAY_BE_REFUSED: usize = 2;

/// The SHA-256 over the serialised records and library documents of a batch: equal for two equal batches.
pub fn batch_digest(batch: &ew_api::import::ImportBatch) -> String {
    let mut h = Sha256::new();
    for r in &batch.records {
        h.update(serde_json::to_vec(r).unwrap());
    }
    for l in &batch.lib {
        h.update(serde_json::to_vec(l).unwrap());
    }
    format!("{:x}", h.finalize())
}

#[test]
#[ignore = "needs the QET examples (EW_QET_EXAMPLES)"]
fn the_qet_examples_import_without_loss_and_the_same_way_twice() {
    let Some(dir) = std::env::var_os("EW_QET_EXAMPLES").map(PathBuf::from) else {
        eprintln!("EW_QET_EXAMPLES is not set; skipped");
        return;
    };
    let library = StarterLibrary::from_dir(&view::repo().join("content")).expect("the starter content loads");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the examples folder is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("qet")))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .qet file in EW_QET_EXAMPLES");
    let (mut refused, mut seen_lossless) = (Vec::new(), 0);
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(path).unwrap();
        let first = match import_qet_batch(&name, bytes.clone(), &library) {
            Ok(b) => b,
            Err(e) => {
                println!("{name}: refused: {e}");
                refused.push(name);
                continue;
            }
        };
        let second = import_qet_batch(&name, bytes, &library).expect("a second import of an accepted file");
        assert_eq!(
            batch_digest(&first),
            batch_digest(&second),
            "{name}: two imports differ"
        );
        let lost = first.report.stats.lost_connections;
        println!("{name}: {} records, {lost} lost connections", first.records.len());
        if LOSSLESS.contains(&name.as_str()) {
            seen_lossless += 1;
            assert_eq!(lost, 0, "{name}: connections lost");
        }
    }
    assert!(
        refused.len() <= MAY_BE_REFUSED,
        "{} of {} examples refused: {refused:?}",
        refused.len(),
        files.len()
    );
    assert_eq!(
        seen_lossless,
        LOSSLESS.len(),
        "the two named projects are in the folder and import"
    );
}
