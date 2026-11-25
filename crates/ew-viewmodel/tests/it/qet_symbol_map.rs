// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File ▸ Import… reads the symbol map beside a QElectroTech file (doc 14 §4.17): the import card says it was
//! read, the imported element draws the starter function its row names, and a wrong row refuses the import in words.

use std::path::{Path, PathBuf};

use crate::common::Tmp;
use ew_testdir::{CONTENT, SAMPLE};
use ew_viewmodel::shell::{Mode, Outcome, Resources, ShellModel};
use serde_json::json;

use crate::common::done;

/// The tutorial's QElectroTech file (a switch, a lamp, a supply and an earth) in the temporary folder, with `map`
/// beside it.
fn set(tmp: &Tmp, map: &str) -> PathBuf {
    let qet = tmp.0.join("lamp-circuit.qet");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/book/src/tutorials/files/lamp-circuit.qet"),
        &qet,
    )
    .unwrap();
    std::fs::write(tmp.0.join("symbol-map.csv"), map).unwrap();
    qet
}

fn shell(tmp: &Tmp) -> ShellModel {
    let resources = Resources {
        sample_dir: Some(PathBuf::from(SAMPLE)),
        content_dir: Some(PathBuf::from(CONTENT)),
        docs_dir: None,
        notices_file: None,
        privacy_file: None,
    };
    ShellModel::new(Mode::Headless, tmp.config().with_resources(resources))
}

fn path(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn the_import_card_says_the_map_beside_the_file_was_read_and_its_row_binds() {
    let tmp = Tmp::new("qet-map");
    let qet = set(&tmp, "element,function,cps\nswitch.elmt,ew.fn.contact-no,13 14\n");
    let mut s = shell(&tmp);
    let dir = tmp.0.join("imported.ew");
    done(
        &mut s,
        "file.import_qet",
        json!({ "file": path(&qet), "dir": path(&dir) }),
    );
    let dialog = s.dialog().expect("the import card").clone();
    let spec = ew_viewmodel::dialogs::spec(&dialog, &s);
    assert!(
        spec.body
            .iter()
            .any(|l| l.contains("symbol-map.csv beside the file was read (1 row)")),
        "{:?}",
        spec.body
    );
    done(&mut s, "file.import_qet_apply", json!({ "confirm": true }));
    s.shutdown();
    let (project, _) = ew_api::Session::open_read_only(&dir, "0.0.0-test").expect("the imported project");
    let q = project.query();
    let contact = q
        .fndefs()
        .iter()
        .find(|(_, c)| {
            c.item
                .meta
                .key
                .as_ref()
                .is_some_and(|k| k.as_str() == "ew.fn.contact-no")
        })
        .map(|(id, _)| id)
        .expect("the starter contact travels with the import");
    assert!(
        q.functions().iter().any(|(_, f)| f.def == contact),
        "the switch draws the starter contact"
    );
}

#[test]
fn a_wrong_row_refuses_the_import_in_words() {
    let tmp = Tmp::new("qet-map-wrong");
    let qet = set(&tmp, "element,function\nswitch.elmt,ew.fn.lamp\n");
    let mut s = shell(&tmp);
    let dir = tmp.0.join("imported.ew");
    match s.dispatch(
        "file.import_qet",
        &json!({ "file": path(&qet), "dir": path(&dir) }),
    ) {
        Outcome::Failed { message } => {
            assert!(
                message.contains("symbol-map.csv, line 2, column `function`"),
                "{message}"
            );
            assert!(message.contains("the map binds coils, contacts"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    assert!(!dir.exists(), "nothing is written");
    s.shutdown();
}
