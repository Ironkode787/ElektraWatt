// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A QElectroTech element collection into a user library (FR-SYM-029): every element a symbol under
//! `LicenseRef-QET-elements` with `foreign_terms`, its source path kept, a file that cannot be converted listed, a
//! second run writing nothing new. The elements are the synthetic ones of `ew-import-qet`'s tests.

use crate::common::{self, Dir};

use std::path::Path;

use ew_api::library::{LibraryTiers, Registry};
use ew_api::qet_collection::{self, LICENCE};
use ew_schema::{Kind, LibDocument};

fn collection_dir() -> (Dir, usize) {
    let root = Dir(ew_testdir::fresh_dir("qet-collection"));
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ew-import-qet/tests/fixtures");
    let mut n = 0;
    for (i, e) in std::fs::read_dir(from).unwrap().enumerate() {
        let path = e.unwrap().path();
        if path.extension().is_some_and(|x| x == "elmt") {
            let folder = root.0.join("electric").join(format!("{}_group", i % 2));
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::copy(&path, folder.join(path.file_name().unwrap())).unwrap();
            n += 1;
        }
    }
    std::fs::write(root.0.join("ELEMENTS.LICENSE"), "licence text").unwrap();
    std::fs::write(root.0.join("electric").join("broken.elmt"), "not xml at all").unwrap();
    (root, n)
}

#[test]
fn a_collection_becomes_symbols_under_the_qet_licence_and_a_second_run_adds_nothing() {
    let (dir, elements) = collection_dir();
    assert!(elements > 0);
    let collection = qet_collection::read_collection(&dir.0).unwrap();
    assert!(collection.licence.is_some());
    let root = Dir(ew_testdir::fresh_dir("qet-collection-lib"));
    let tiers = LibraryTiers {
        company: vec![root.0.join("company")],
        ..common::tiers(&root.0)
    };
    let mut reg = Registry::open(&tiers).unwrap();
    let lib = reg
        .create(&root.0.join("company").join("qet.ewlib"), "qet", "QET", LICENCE)
        .unwrap();
    let (done, write) = qet_collection::import_into_library(&mut reg, lib, &collection, false).unwrap();
    assert_eq!(done.symbols, elements);
    assert_eq!(done.refused.len(), 1);
    assert!(done.refused[0].0.ends_with("broken.elmt"), "{:?}", done.refused);
    assert!(write.is_some_and(|w| !w.added.is_empty()));
    let symbols = reg.documents(lib, Kind::Symbol);
    assert_eq!(symbols.len(), elements);
    for doc in &symbols {
        let LibDocument::Symbol(s) = doc else {
            unreachable!()
        };
        let meta = &s.item.meta;
        assert_eq!(meta.licence.as_deref(), Some(LICENCE));
        assert!(meta.foreign_terms);
        assert!(
            meta.sources.iter().any(|x| x.starts_with("qet.path:electric/")),
            "{:?}",
            meta.sources
        );
        assert!(meta.sources.iter().any(|x| x.starts_with("qet.licence:ELEMENTS.LICENSE@sha256:")));
        assert_eq!(s.item.grid, ew_units::Grid::QET_10PX, "the QET grid is kept");
    }
    let (_, again) = qet_collection::import_into_library(&mut reg, lib, &collection, false).unwrap();
    let again = again.expect("the documents are written again as they are");
    assert!(again.added.is_empty() && again.updated.is_empty());
}
