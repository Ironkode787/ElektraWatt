// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN data imported through `file.import_eplan` on a writable copy of the sample (WP-S3.18, doc 14 §5): the Data
//! Portal terms notice cannot be skipped. Not here: File ▸ Import…, its steps and its card in the window (`ew-ui`'s
//! `eplan_import.rs`).

use crate::common::{SAMPLE, Tmp, copy_dir, done, library_tiers};

use ew_viewmodel::shell::{Dialog, EplanStep, Mode, ShellModel};
use serde_json::json;
use std::path::Path;

const EDZ: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

/// The terms notice cannot be skipped: an EDZ under another name, and an import that names its step before the notice
/// was shown, stop at the notice, and nothing is written.
#[test]
fn an_edz_under_another_name_and_a_named_step_still_stop_at_the_terms() {
    let tmp = Tmp::new("eplan-terms");
    let disguised = tmp.0.join("psu.xml");
    std::fs::copy(EDZ, &disguised).unwrap();
    let config = tmp.config().with_libraries(Some(library_tiers(&tmp.0)));
    let mut s = ShellModel::new(Mode::Gui, config);
    let project = tmp.sample_copy();
    copy_dir(Path::new(SAMPLE), &project);
    done(&mut s, "file.open", json!({"dir": project.to_string_lossy()}));
    for how in ["library", "update"] {
        done(
            &mut s,
            "file.import_eplan",
            json!({"file": disguised.to_string_lossy(), "how": how}),
        );
        // The file is read as a job (FT-1763): its end opens the notice.
        s.wait_jobs();
        assert!(
            matches!(
                s.dialog(),
                Some(Dialog::ImportEplan {
                    step: EplanStep::Terms,
                    ..
                })
            ),
            "{how}: {:?}",
            s.dialog()
        );
    }
    assert!(
        !tmp.0
            .join("data")
            .join("libraries")
            .join("user-parts.ewlib")
            .exists()
    );
    s.shutdown();
}
