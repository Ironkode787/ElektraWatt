// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 2 in the Session: `SessionOptions.libraries`, `Session::libraries`, `Session::part_search` with the
//! project's held parts, the starter's library ID, and the ignored NFR-PERF-21 test (IX-9).

use std::path::PathBuf;

use ew_api::library::{self, LibraryTiers, PartQuery, Registry, Tier};
use ew_api::{Session, SessionOptions};
use ew_ids::Uuid;
use ew_schema::{LibDocument, LibRef};
use serde_json::json;

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiers(root: &std::path::Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

fn session(libraries: Option<LibraryTiers>, lib: Vec<LibDocument>) -> Session {
    let project = serde_json::from_str(PROJECT).unwrap();
    Session::from_records(
        project,
        [],
        lib,
        SessionOptions {
            libraries,
            ..SessionOptions::default()
        },
    )
    .unwrap()
}

/// The starter's compiled-in library ID is `ew_interop_api::SYSTEM_LIBRARY` (FT-24): `ew-library` cannot name
/// `ew-interop-api`, so the equality is asserted here, where both are visible.
#[test]
fn the_starter_library_is_the_system_library() {
    assert_eq!(
        library::starter::LIBRARY,
        ew_import_qet::ew_interop_api::SYSTEM_LIBRARY
    );
    assert_eq!(
        library::starter_parts_library(),
        Uuid::new_v5(
            &ew_import_qet::ew_interop_api::SYSTEM_LIBRARY,
            b"ew-starter-parts"
        )
    );
}

/// Without tiers, and with tiers that cannot be opened, the session has the starter and the compiled-in starter
/// parts (WP-S2.18 D5), read from no file; only the tiers that cannot be opened report why.
#[test]
fn tiers_that_cannot_be_opened_fall_back_to_the_starter() {
    struct Case {
        name: &'static str,
        unopenable: bool,
    }
    for c in [
        Case {
            name: "without tiers",
            unopenable: false,
        },
        Case {
            name: "tiers that cannot be opened",
            unopenable: true,
        },
    ] {
        let root = Dir(ew_testdir::fresh_dir("ew-api-libraries-broken"));
        let t = c.unopenable.then(|| {
            std::fs::create_dir_all(&root.0).unwrap();
            let mut t = tiers(&root.0);
            // The cache directory is a file.
            t.cache = root.0.join("cache-is-a-file");
            std::fs::write(&t.cache, b"x").unwrap();
            t
        });
        let s = session(t, Vec::new());
        let libs = s.libraries().libraries();
        assert_eq!(libs.len(), 2, "case {}", c.name);
        assert_eq!(
            (libs[0].key.as_str(), libs[0].tier),
            ("ew-starter", Tier::System),
            "case {}",
            c.name
        );
        assert_eq!(
            (libs[1].id, libs[1].tier),
            (library::starter_parts_library(), Tier::System),
            "case {}",
            c.name
        );
        assert_eq!(s.library_error().is_some(), c.unopenable, "case {}", c.name);
        if !c.unopenable {
            assert!(s.libraries().index_path().is_none(), "case {}", c.name);
            let hits = s.part_search(&PartQuery::default());
            assert!(!hits.is_empty(), "case {}", c.name);
            assert!(
                hits.iter().all(|h| h.library == library::starter_parts_library()),
                "case {}: the starter parts only",
                c.name
            );
        }
    }
}

/// `part_search` ranks the parts the project holds first inside their rank tier.
#[test]
fn part_search_ranks_held_parts_first() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-libraries"));
    let t = tiers(&root.0);
    let mut s0 = session(Some(t.clone()), Vec::new());
    let reg: &mut Registry = s0.libraries_mut();
    let lib = reg
        .create(
            &root.0.join("company").join("std.ewlib"),
            "std",
            "Standard",
            "CC0-1.0",
        )
        .unwrap();
    let docs = ew_testkit::library::generated_parts(2, lib);
    reg.write(lib, &docs).unwrap();
    // The manufacturer's alias finds its part: one manufacturer per part for n = 2.
    let q = PartQuery {
        text: "Maker00".into(),
        ..PartQuery::default()
    };
    assert_eq!(fixture_hits(&s0, &q).len(), 1);
    let all = PartQuery {
        text: String::new(),
        ..PartQuery::default()
    };
    let order: Vec<Uuid> = fixture_hits(&s0, &all).iter().map(|h| h.item).collect();
    assert_eq!(order.len(), 2);
    // A project holding the second part: it comes first.
    let second = order[1];
    let r: LibRef = serde_json::from_value(json!({
        "library": lib.to_string(), "item": second.to_string(), "version_req": "^1"
    }))
    .unwrap();
    let closure = s0.libraries().closure(&r).unwrap();
    let s1 = session(Some(t), closure);
    let held: Vec<Uuid> = fixture_hits(&s1, &all).iter().map(|h| h.item).collect();
    assert_eq!(held, [second, order[0]]);
    assert!(s1.library_error().is_none());
    assert_eq!(s1.libraries().libraries()[1].index, library::IndexState::Fresh);
}

/// The hits of `q` outside the compiled-in starter parts (WP-S2.18 D5), which every session's registry holds.
fn fixture_hits(s: &Session, q: &PartQuery) -> Vec<library::PartHit> {
    s.part_search(q)
        .into_iter()
        .filter(|h| h.library != library::starter_parts_library())
        .collect()
}

/// IX-9 and NFR-PERF-21: the first 50 results within 100 ms and a full rebuild within 60 s over a 500,000-part
/// library generated by `ew-testkit`.
#[test]
#[ignore = "NFR-PERF-21 release-profile performance test; not run during development (owner's rule)"]
// The measurement itself: the wall clock times the rebuild and the searches; nothing else depends on it.
#[allow(clippy::disallowed_methods)]
fn nfr_perf_21_part_search_over_500k_parts() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-libraries-perf"));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&root.0.join("company").join("big.ewlib"), "big", "Big", "CC0-1.0")
        .unwrap();
    reg.write(lib, &ew_testkit::library::generated_parts(500_000, lib))
        .unwrap();
    let started = std::time::Instant::now();
    reg.rebuild_index().unwrap();
    let rebuild = started.elapsed();
    assert!(rebuild.as_secs_f64() <= 60.0, "rebuild in {rebuild:?}");
    for text in [
        "AB0000123C16",
        "breaker",
        "relay",
        "circuit breaker",
        "Maker07",
        "0012",
    ] {
        let started = std::time::Instant::now();
        let hits = reg.search(&PartQuery {
            text: text.into(),
            limit: 50,
            ..PartQuery::default()
        });
        let took = started.elapsed();
        assert!(!hits.is_empty(), "{text}");
        assert!(took.as_secs_f64() <= 0.1, "{text}: {took:?}");
    }
}
