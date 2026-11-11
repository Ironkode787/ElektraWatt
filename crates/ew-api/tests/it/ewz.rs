// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the session, `Session::export_ewz` packs the saved generation,
//! never the dirty model, and says so; `Session::open_ewz` (and every reader through `Session::open_reader`) opens an
//! `.ewz` read-only from memory; a refused archive is `OpenError::Ewz`, in words.

use std::path::Path;

use ew_api::{Access, AnyCommand, OpenError, ReadOnlyReason, Session, SessionOptions, StoreOptions};

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

fn page(number: u32) -> AnyCommand {
    AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "number": number }),
    }
}

/// A saved project of one page at `dir`, and its editing session.
fn saved(dir: &Path) -> Session {
    let mut session = Session::from_records(
        serde_json::from_str(PROJECT).unwrap(),
        [],
        [],
        SessionOptions::default(),
    )
    .unwrap();
    session.execute(&page(1)).unwrap();
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    session.create(dir, store).unwrap();
    session
}

#[test]
fn a_dirty_session_exports_its_saved_generation_and_says_so() {
    let base = ew_testdir::fresh_dir("ew-api-ewz-export");
    let dir = base.join("p.ew");
    let mut session = saved(&dir);
    let clean = session.export_ewz().unwrap();
    assert!(!clean.unsaved);
    session.execute(&page(2)).unwrap();
    let dirty = session.export_ewz().unwrap();
    assert!(dirty.unsaved, "the session holds an unsaved page");
    assert_eq!(
        dirty.bytes, clean.bytes,
        "the saved generation, not the model in memory"
    );
    assert_eq!((dirty.generation, dirty.files), (1, clean.files));
    session.close();

    let (reader, _) = Session::open_reader(&dir, "ElektraWatt test", "export ewz").unwrap();
    assert_eq!(reader.export_ewz().unwrap().bytes, clean.bytes);

    // The archive opens read-only from memory through every reader's open, and packs to the same bytes again.
    let archive = base.join("p.ewz");
    std::fs::write(&archive, &clean.bytes).unwrap();
    for (opened, report) in [
        Session::open_ewz(&archive, "ElektraWatt test").unwrap(),
        Session::open_reader(&archive, "ElektraWatt test", "check").unwrap(),
        Session::open_read_only(&archive, "ElektraWatt test").unwrap(),
    ] {
        assert_eq!(report.access, Access::ReadOnly(ReadOnlyReason::Requested));
        assert_eq!(report.generation, 1);
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        assert!(!report.journaled, "no journal");
        assert_eq!(opened.export_ewz().unwrap().bytes, clean.bytes);
    }
    assert_eq!(
        std::fs::read_dir(&base).unwrap().count(),
        2,
        "nothing written beside the project and the archive"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_refused_archive_is_an_open_error_in_words() {
    let base = ew_testdir::fresh_dir("ew-api-ewz-refused");
    let archive = base.join("p.ewz");
    std::fs::write(&archive, b"PK not an archive at all, but long enough").unwrap();
    let Err(e) = Session::open_reader(&archive, "ElektraWatt test", "check") else {
        panic!("a damaged archive opens");
    };
    assert!(matches!(e, OpenError::Ewz(_)));
    assert_eq!(
        e.to_string(),
        "the .ewz is refused: not an archive of the expected form: no end record at the end (an archive comment or \
         trailing bytes)"
    );
    let Err(e) = Session::open_ewz(&base.join("missing.ewz"), "ElektraWatt test") else {
        panic!("a missing archive opens");
    };
    assert!(matches!(e, OpenError::Ewz(ew_api::ewz::EwzError::Io(_))));
    let _ = std::fs::remove_dir_all(&base);
}
