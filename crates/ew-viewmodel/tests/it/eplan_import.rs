// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN parts files through the doors of Parts (doc 07 §3.6, doc 14 §5) on a writable copy of the sample: the Data
//! Portal terms notice cannot be skipped, whichever door; past it the file's parts stand in Parts as a set against My
//! parts, and Save writes them there. Not here: the window's steps (`ew-ui`'s `eplan_import.rs`).

use crate::common::{SAMPLE, Tmp, copy_dir, done, library_tiers};

use ew_viewmodel::shell::{Dialog, EplanStep, Mode, ShellModel};
use serde_json::json;
use std::path::Path;

const EDZ: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-import-eplan/tests/fixtures/synthetic-psu-7z.edz"
);

/// The sample's copy open in the window, its person's user folder under `tmp`.
fn on_sample(tmp: &Tmp) -> ShellModel {
    let config = tmp.config().with_libraries(Some(library_tiers(&tmp.0)));
    let mut s = ShellModel::new(Mode::Gui, config);
    let project = tmp.sample_copy();
    copy_dir(Path::new(SAMPLE), &project);
    done(&mut s, "file.open", json!({"dir": project.to_string_lossy()}));
    s
}

/// The terms notice cannot be skipped: an EDZ under another name stops at the notice through File ▸ Import… and through
/// Import… of Parts, and nothing is written.
#[test]
fn an_edz_under_another_name_stops_at_the_terms_through_either_door() {
    let tmp = Tmp::new("eplan-terms");
    let disguised = tmp.0.join("psu.xml");
    std::fs::copy(EDZ, &disguised).unwrap();
    let mut s = on_sample(&tmp);
    let file = disguised.to_string_lossy().into_owned();
    for (action, params) in [
        ("file.import_eplan", json!({ "file": file })),
        ("parts.import", json!({ "path": file })),
    ] {
        done(&mut s, action, params);
        // The file is read as a job: its end opens the notice.
        s.wait_jobs();
        assert!(
            matches!(
                s.dialog(),
                Some(Dialog::ImportEplan {
                    step: EplanStep::Terms,
                    ..
                })
            ),
            "{action}: {:?}",
            s.dialog()
        );
        done(&mut s, "ui.dismiss", json!({}));
    }
    assert!(s.import_set().is_none(), "no set before the notice");
    assert!(
        !tmp.0
            .join("data")
            .join("libraries")
            .join("user-parts.ewlib")
            .exists()
    );
    // The notice's Continue closes it, and the file's set stands in Parts.
    done(&mut s, "file.import_eplan", json!({ "file": file }));
    s.wait_jobs();
    done(
        &mut s,
        "file.import_eplan",
        json!({ "file": file, "terms": true }),
    );
    s.wait_jobs();
    assert!(s.dialog().is_none(), "{:?}", s.dialog());
    assert!(s.import_set().is_some());
    s.shutdown();
}

/// The project's copy of the file's part is offered its update from the set (FR-PRT-014): the update's card lists what
/// changes before anything does.
#[test]
fn the_project_s_copy_of_an_edz_s_part_is_updated_from_its_set() {
    let tmp = Tmp::new("eplan-update");
    let mut s = on_sample(&tmp);
    // The sample holds no copy of the EDZ's part: the test tables' row gives it one whose fields the EDZ changes.
    let table = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tables/parts.csv"),
    )
    .unwrap();
    let row = table
        .lines()
        .find(|l| l.contains("2904602"))
        .expect("the test tables' row");
    crate::common::import_parts(&mut s, &tmp, &format!("{row}\n"));
    done(&mut s, "parts.import", json!({"path": EDZ, "terms": true}));
    s.wait_jobs();
    assert_eq!(s.import_set().and_then(|i| i.project), Some((1, 1)));
    done(&mut s, "parts.import", json!({"update": true}));
    assert!(s.properties().card().is_some(), "the update's card");
    s.shutdown();
}

/// Past the notice the file's parts stand in Parts as a set against My parts; Save writes them there, and the file read
/// again brings them as the same.
#[test]
fn an_edz_s_parts_come_into_parts_as_a_set_and_save_writes_my_parts() {
    let tmp = Tmp::new("eplan-set");
    let mut s = on_sample(&tmp);
    done(&mut s, "parts.import", json!({"path": EDZ, "terms": true}));
    s.wait_jobs();
    let set = s.import_set().and_then(|i| i.set.clone()).expect("the set");
    // The terms notice it was read past is logged first in its report (EW-IMP-201, doc 14 §5).
    assert_eq!(set.import.report.entries.first().map(|e| e.code.number()), Some(201));
    let (new, changed, _, _) = set.counts();
    assert!(new > 0 && changed == 0, "{:#?}", set.rows);
    assert!(
        s.import_set().unwrap().map.is_none(),
        "an EPLAN file has no columns to map"
    );
    done(&mut s, "parts.save", json!({}));
    assert!(
        tmp.0
            .join("data")
            .join("libraries")
            .join("user-parts.ewlib")
            .exists()
    );
    done(&mut s, "parts.import", json!({"path": EDZ}));
    s.wait_jobs();
    let again = s
        .import_set()
        .and_then(|i| i.set.clone())
        .expect("the set read again");
    let (new, changed, same, _) = again.counts();
    assert_eq!((new, changed), (0, 0), "{:#?}", again.rows);
    assert!(same > 0);
    s.shutdown();
}

/// An older script's step of an EPLAN parts file (`how` `update` or `library`, which Parts took over) is refused in words
/// that name the way there, not done otherwise without a word (review finding 10 of lane 3).
#[test]
fn an_older_script_s_parts_step_is_refused_with_the_way_into_parts() {
    let tmp = Tmp::new("eplan-how-old");
    let mut s = on_sample(&tmp);
    for how in ["update", "library"] {
        assert_eq!(
            crate::common::refused(&mut s, "file.import_eplan", json!({"file": EDZ, "how": how})),
            "reason-eplan-how-parts"
        );
    }
    assert!(s.import_set().is_none());
    s.shutdown();
}
