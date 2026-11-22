// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Add to my parts library as a read (FT-2081): the registry's part is taken first, the file parsed against it off the
//! registry's thread and written at the end, into the library as it is then.

use crate::common::{self, Dir};

use std::path::Path;

use ew_api::catalog::{ImportInput, LibraryPart};
use ew_api::eplan::{self, FndefMap};
use ew_api::library::{LibraryTiers, Registry};
use ew_derive::ew_ids::Id;
use ew_schema::{LibDocument, LibRef};

/// The part 2904602 with the description `description`, as a parts XML.
fn part_xml(description: &str) -> ImportInput {
    let xml = format!(
        r#"<part P_ARTICLE_PARTNR="2904602" P_ARTICLE_MANUFACTURER="Phoenix Contact" P_ARTICLE_DESCR1="{description}">
        <functiontemplate connectionDesignation="L&#10;N" functiondefcategory="2" functiondefgroup="1" functiondefid="1"/></part>"#
    );
    ImportInput::new("2904602.xml", xml.into_bytes()).unwrap()
}

/// A part changed by another write while the file was parsed is written as the next patch version of the copy held at
/// the write, with the file's content: the version the parse proposed is taken meanwhile.
#[test]
fn a_write_after_a_change_meanwhile_bumps_against_the_library_as_it_is_then() {
    let root = Dir(ew_testdir::fresh_dir("library-write-meanwhile"));
    let tiers = LibraryTiers {
        company: vec![root.0.join("company")],
        ..common::tiers(&root.0)
    };
    let mut reg = Registry::open(&tiers).unwrap();
    let lib = reg
        .create(
            &root.0.join("company").join("mine.ewlib"),
            "mine",
            "Mine",
            "NOASSERTION",
        )
        .unwrap();
    let edz = ImportInput::new(
        "synthetic-psu-zip.edz",
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../ew-import-eplan/tests/fixtures/synthetic-psu-zip.edz"),
        )
        .unwrap(),
    )
    .unwrap();
    let fndefs = FndefMap::shipped();
    let first =
        eplan::import_into_library(&mut reg, lib, &edz, &fndefs, (None, false, false), false).unwrap();
    let item = match &first.import.parts[0] {
        LibDocument::Part(p) => p.item_id,
        _ => unreachable!(),
    };
    let held = |reg: &Registry| match reg
        .resolve(&LibRef {
            library: Id::from_uuid(lib),
            item: Id::from_uuid(item.as_uuid()),
            version_req: "*".into(),
            extra: Default::default(),
        })
        .unwrap()
    {
        LibDocument::Part(p) => p,
        _ => unreachable!(),
    };
    let before = held(&reg).version;

    // The read starts: the registry's part is taken and the file parsed against it.
    let part = LibraryPart::take(&reg, lib).unwrap();
    let parsed = eplan::parse_for_library(
        &part,
        &part_xml("Power supply, mine"),
        &fndefs,
        (None, true),
        false,
    )
    .unwrap();
    // Meanwhile another import writes the part's next version.
    eplan::import_into_library(
        &mut reg,
        lib,
        &part_xml("Power supply, theirs"),
        &fndefs,
        (None, true, false),
        false,
    )
    .unwrap();
    let meanwhile = held(&reg).version;
    assert_ne!(meanwhile, before);

    part.write(&mut reg, parsed, false)
        .expect("written over the library as it is now");
    let after = held(&reg);
    assert_eq!(
        Some(after.version.as_str()),
        ew_import_catalog::bump_patch(&meanwhile).as_deref(),
        "the next patch version of the copy held at the write"
    );
    assert!(
        serde_json::to_string(&after.item.description)
            .unwrap()
            .contains("mine"),
        "the file's content"
    );
}
