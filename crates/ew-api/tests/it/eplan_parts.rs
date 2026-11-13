// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EPLAN part data through the host (doc 14 §5.2, §5.3): AC-38 (an EDZ for a golden part updates the project's copy
//! through a previewed command that lists the changed fields), the library write, the function-definition table.

use crate::common::{self, Dir};
use crate::golden_common;

use std::path::Path;

use ew_api::catalog::{CatalogError, ImportInput};
use ew_api::eplan::{self, FndefMap};
use ew_api::library::{LibraryTiers, Registry};
use ew_api::{Session, SessionOptions};
use ew_derive::ew_ids::{Id, Uuid};
use ew_import_catalog::ew_interop_api::SYSTEM_LIBRARY;
use ew_schema::{LibDocument, LibRef};
use serde_json::json;

/// A synthetic EDZ of the repository's own making (`ew-import-eplan`'s fixtures): the part 2904602.
fn fixture(name: &str) -> ImportInput {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-import-eplan/tests/fixtures")
        .join(name);
    ImportInput::new(name, std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn golden() -> Session {
    let batch = golden_common::batch();
    let (project, records) = common::split(&batch);
    Session::from_records(project, records, batch.lib.clone(), SessionOptions::default()).unwrap()
}

fn held_part(s: &Session, number: &str) -> ew_schema::LibCopy<ew_schema::Part> {
    let q = s.query();
    q.parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(_, c)| (**c).clone())
        .expect("a golden part")
}

/// AC-38: an EDZ for the golden part 2904602 updates the project's copy through a previewed `library.update` that
/// lists the changed fields and the device that uses the part; what the EDZ does not carry stays; nothing changes
/// before the commit; the commit is one undo step.
#[test]
fn an_edz_updates_the_golden_part_copy_through_a_previewed_command_listing_the_changed_fields() {
    let mut s = golden();
    let before = held_part(&s, "2904602");
    let import = s.eplan_parts(&fixture("synthetic-psu-7z.edz"), &FndefMap::shipped(), None).unwrap();
    let updates = s.update_parts_from_import(&import).unwrap();
    assert_eq!(updates.matched.len(), 1);
    assert!(updates.unmatched.is_empty() && updates.ambiguous.is_empty());
    let plan = updates.plan.expect("the copy differs");
    // The card's field diff.
    let [copy] = plan.copies.as_slice() else {
        panic!("one copy")
    };
    let fields: Vec<&str> = copy.fields.iter().map(|f| f.member.as_str()).collect();
    for changed in ["description", "free_props", "mech", "meta"] {
        assert!(fields.contains(&changed), "{changed} in {fields:?}");
    }
    assert!(!fields.contains(&"tech_data") && !fields.contains(&"function_template"), "{fields:?}");
    assert_eq!((copy.from.as_str(), copy.to.as_str()), ("1.0.0", "1.0.1"));
    assert_eq!(plan.devices.len(), 1);
    // A preview: the model is as it was until the commit.
    assert_eq!(held_part(&s, "2904602"), before);
    let preview = s.preview(&plan.command).unwrap();
    s.commit(preview).unwrap();
    let after = held_part(&s, "2904602");
    assert_eq!(after.version, "1.0.1");
    assert_eq!(
        serde_json::to_value(&after.item.description).unwrap(),
        json!({"de-DE": "Stromversorgung, 1-phasig", "en-US": "Power supply, single phase"})
    );
    // Kept: the technical data and the pins' potential roles, which the file does not carry.
    assert_eq!(after.item.tech_data, before.item.tech_data);
    let roles = |p: &ew_schema::LibCopy<ew_schema::Part>| -> Vec<_> {
        p.item.cp_pattern.iter().map(|c| (c.idx, c.designation.clone(), c.potential_role.clone())).collect()
    };
    assert_eq!(roles(&after), roles(&before));
    assert!(after.item.meta.foreign_terms);
    assert!(after.item.extra.contains_key("eplan.accessories"));
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert_eq!(held_part(&s, "2904602"), before);
    // The same file again changes nothing more than its provenance: a second preview lists no copy once committed.
}

/// Parts of the file that the project holds no copy of are listed and never created.
#[test]
fn parts_the_project_does_not_hold_are_listed_not_created() {
    let s = golden();
    let xml = r#"<partsmanagement>
        <part P_ARTICLE_PARTNR="9999-X" P_ARTICLE_MANUFACTURER="Acme" P_ARTICLE_DESCR1="Gadget"/></partsmanagement>"#;
    let input = ImportInput::new("p.xml", xml.as_bytes().to_vec()).unwrap();
    let import = s.eplan_parts(&input, &FndefMap::shipped(), None).unwrap();
    let updates = s.update_parts_from_import(&import).unwrap();
    assert!(updates.plan.is_none() && updates.matched.is_empty());
    assert_eq!(updates.unmatched, ["9999-X"]);
    assert!(s.query().parts().iter().all(|(_, c)| c.item.part_number.as_str() != "9999-X"));
}

/// A library write: the parts of an EDZ go into a user library with their provenance and the vendor terms; a second
/// import of the same file writes nothing; a changed part under its held version is refused unless it is bumped.
#[test]
fn an_edz_is_written_into_a_library_and_a_changed_part_needs_a_bump() {
    let root = Dir(ew_testdir::fresh_dir("eplan-library"));
    let tiers = LibraryTiers {
        company: vec![root.0.join("company")],
        ..common::tiers(&root.0)
    };
    let mut reg = Registry::open(&tiers).unwrap();
    let lib = reg
        .create(&root.0.join("company").join("mine.ewlib"), "mine", "Mine", "NOASSERTION")
        .unwrap();
    let edz = fixture("synthetic-psu-zip.edz");
    let write = (None, false, false);
    let first = eplan::import_into_library(&mut reg, lib, &edz, &FndefMap::shipped(), write, true).unwrap();
    assert_eq!(first.import.counts().new, 1);
    assert_eq!(first.write.as_ref().map(|w| w.added.len()), Some(2), "the part and its manufacturer");
    assert_eq!(first.import.report.entries[0].code.number(), 201, "the notice is logged first");
    let part = match &first.import.parts[0] {
        LibDocument::Part(p) => p.clone(),
        _ => unreachable!(),
    };
    assert!(part.item.meta.foreign_terms);
    assert!(part.item.meta.sources.iter().any(|s| s.contains("synthetic-psu-zip.edz")));
    let again = eplan::import_into_library(&mut reg, lib, &edz, &FndefMap::shipped(), write, false).unwrap();
    assert_eq!(again.import.counts().unchanged, 1);
    // The same part with another description: refused under the held version, written bumped.
    let xml = r#"<part P_ARTICLE_PARTNR="2904602" P_ARTICLE_MANUFACTURER="Phoenix Contact" P_ARTICLE_DESCR1="Power supply, new">
        <functiontemplate connectionDesignation="L&#10;N" functiondefcategory="2" functiondefgroup="1" functiondefid="1"/></part>"#;
    let edited = ImportInput::new("2904602.xml", xml.as_bytes().to_vec()).unwrap();
    let refused = eplan::import_into_library(&mut reg, lib, &edited, &FndefMap::shipped(), write, false);
    assert!(matches!(refused, Err(CatalogError::NeedsBump(_))));
    let bumped =
        eplan::import_into_library(&mut reg, lib, &edited, &FndefMap::shipped(), (None, true, false), false)
            .unwrap();
    assert_eq!(bumped.import.counts().updated, 1);
}

/// Every function definition the shipped table names is one of the starter library; the user's own rows win over the
/// shipped ones and are read from the configuration directory.
#[test]
fn the_function_definition_table_names_starter_items_and_reads_the_users_rows() {
    let reg = Registry::starter_only();
    for key in FndefMap::shipped().fndef_keys() {
        let r = LibRef {
            library: Id::from_uuid(SYSTEM_LIBRARY),
            item: Id::from_uuid(Uuid::new_v5(&SYSTEM_LIBRARY, key.as_bytes())),
            version_req: "*".to_owned(),
            extra: Default::default(),
        };
        assert!(
            matches!(reg.resolve(&r), Ok(LibDocument::Fndef(_))),
            "{key} is not a starter function definition"
        );
    }
    let dir = Dir(ew_testdir::fresh_dir("eplan-fndef-map"));
    assert!(eplan::fndef_map(Some(&dir.0)).is_ok());
    std::fs::create_dir_all(dir.0.join("imports")).unwrap();
    std::fs::write(
        dir.0.join("imports").join(eplan::FNDEF_MAP_FILE),
        "[[map]]\ncategory = \"1\"\ngroup = \"1\"\nid = \"1\"\nfndef = \"ew.fn.terminal-pe\"\n",
    )
    .unwrap();
    let mine = eplan::fndef_map(Some(&dir.0)).unwrap();
    assert!(mine.fndef_keys().contains(&"ew.fn.terminal-pe"));
    std::fs::write(dir.0.join("imports").join(eplan::FNDEF_MAP_FILE), "not a table [").unwrap();
    assert!(eplan::fndef_map(Some(&dir.0)).is_err());
}
