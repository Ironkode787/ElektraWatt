// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 4 in the Session (D11): the column map saved per source key, the import into a project through
//! `parts.import` (IM-1), and the import into a writable library (IM-6): a second import of a changed list updates
//! in place, changed content under a held version is refused unless bumped, and the library lock is taken over only
//! when it is stale and the caller says so. The parts are invented generic data and public catalogue numbers.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ew_api::catalog::{
    self, CatalogError, ColumnMap, LibraryImport, RowStatus, import_into_library, load_column_map,
    read_input, save_column_map,
};
use ew_api::library::lock::{Identity, LOCK};
use ew_api::library::{LibraryError, LibraryTiers, Registry};
use ew_api::{Session, SessionOptions};
use ew_ids::Uuid;
use ew_import_catalog::ew_interop_api::Limits;
use ew_schema::{Date, LibDocument, LibRef};
use serde_json::json;

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

fn session(root: &Path) -> Session {
    let project = serde_json::from_str(PROJECT).unwrap();
    Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            libraries: Some(tiers(root)),
            ..SessionOptions::default()
        },
    )
    .unwrap()
}

/// A manufacturer of the master list, written into `library`.
fn manufacturer(library: Uuid, key: &str, name: &str, aliases: &[&str]) -> LibDocument {
    let item = Uuid::new_v5(&library, key.as_bytes());
    LibDocument::from_value(json!({
        "$k": "manufacturer",
        "id": ew_ids::v5::lib_copy_id::<()>(library, item).as_uuid().to_string(),
        "library_id": library.to_string(),
        "item_id": item.to_string(),
        "version": "1.0.0",
        "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "key": key,
        "name": name,
        "aliases": aliases,
    }))
    .unwrap()
}

/// A company library `std` holding the master list (Schneider Electric with its alias, Phoenix Contact).
fn company_library(reg: &mut Registry, root: &Path) -> Uuid {
    let lib = reg
        .create(
            &root.join("company").join("std.ewlib"),
            "std",
            "Standard",
            "CC0-1.0",
        )
        .unwrap();
    reg.write(
        lib,
        &[
            manufacturer(lib, "SE", "Schneider Electric", &["Schneider"]),
            manufacturer(lib, "PXC", "Phoenix Contact", &[]),
        ],
    )
    .unwrap();
    lib
}

/// A supplier list with its own headers (English, comma-separated).
const LIST: &str = "Maker,Order code,Type,Text,Category,Rated current\n\
Schneider,A9P52610,iC40N 1P+N C10,Circuit breaker,device,10 A\n\
Phoenix Contact,3209510,PT 2.5,Feed-through terminal,terminal,\n\
Generic Panels,GP-100,,Panel module,device,\n\
Phoenix Contact,,,No number,,\n";

/// The same list a year later: one text changed, one part added.
const LIST_CHANGED: &str = "Maker,Order code,Type,Text,Category,Rated current\n\
Schneider,A9P52610,iC40N 1P+N C10,Miniature circuit breaker,device,10 A\n\
Phoenix Contact,3209510,PT 2.5,Feed-through terminal,terminal,\n\
Generic Panels,GP-100,,Panel module,device,\n\
Generic Panels,GP-200,,Panel module (large),device,\n";

fn map() -> ColumnMap {
    ColumnMap::new(
        None,
        [
            ("manufacturer", "Maker"),
            ("part_number", "Order code"),
            ("type_designation", "Type"),
            ("description", "Text"),
            ("kind", "Category"),
            ("tech_data.i_n", "Rated current"),
        ],
    )
    .unwrap()
}

fn date(s: &str) -> Option<Date> {
    Some(s.parse().unwrap())
}

fn write_file(root: &Path, name: &str, text: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

fn statuses(i: &LibraryImport) -> Vec<(String, String)> {
    i.import
        .rows
        .iter()
        .map(|r| {
            let s = match &r.status {
                RowStatus::New => "new".to_owned(),
                RowStatus::Updated { from, version } => format!("updated {from} -> {version}"),
                RowStatus::Unchanged => "unchanged".to_owned(),
                RowStatus::Duplicate { of_line } => format!("duplicate of {of_line}"),
                RowStatus::Rejected(why) => format!("rejected: {why}"),
            };
            (r.part_number.clone(), s)
        })
        .collect()
}

/// The part `item` of `library` as the registry resolves it at any version: (version, description JSON).
fn held(reg: &Registry, library: Uuid, item: Uuid) -> (String, serde_json::Value) {
    let r = LibRef {
        library: ew_ids::Id::from_uuid(library),
        item: ew_ids::Id::from_uuid(item),
        version_req: "*".into(),
        extra: Default::default(),
    };
    match reg.resolve(&r).unwrap() {
        LibDocument::Part(p) => (
            p.version.clone(),
            serde_json::to_value(&p.item).unwrap()["description"].clone(),
        ),
        other => panic!("not a part: {:?}", other.kind()),
    }
}

/// IM-1: a list with other headers imports into a project through the map saved for its source; `parts.import`
/// creates the new parts in one step, and a second run finds them held.
#[test]
fn a_list_imports_into_a_project_through_its_saved_map() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-catalog-project"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut s = session(&root.0);
    company_library(s.libraries_mut(), &root.0);

    let config = root.0.join("config");
    let saved = save_column_map(&config, "supplier-a", &map()).unwrap();
    assert_eq!(saved, config.join("imports").join("supplier-a.toml"));
    let loaded = load_column_map(&config, "supplier-a").unwrap();
    assert_eq!(loaded, map(), "the saved map reads back");
    assert!(matches!(
        catalog::map_path(&config, "../escape"),
        Err(CatalogError::SourceKey(_))
    ));

    let input = read_input(&write_file(&root.0, "supplier-a.csv", LIST), &Limits::default()).unwrap();
    let master = catalog::manufacturer_master(s.libraries());
    // The company library's two manufacturers and the system library's "Generic" (WP-S2.18 D5).
    assert_eq!(master.len(), 3, "the master list of every tier");
    let (import, cmd) = s
        .catalog_parts(&input, &loaded, &master, date("2026-10-29"))
        .unwrap();
    let c = import.counts();
    assert_eq!((c.new, c.rejected), (3, 1), "{:?}", import.rows);
    let cmd = cmd.expect("a parts.import command");
    assert_eq!(cmd.name, "parts.import");
    let preview = s.preview(&cmd).unwrap();
    s.commit(preview).unwrap();
    let q = s.query();
    let project = q.project().id.as_uuid();
    let parts = q.parts();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|(_, p)| p.library_id.as_uuid() == project));
    // The manufacturers the parts name are embedded as project copies (WP-S1.19 D10), the master's two and the new
    // one, and the parts print their names; no reference keeps a `key`.
    assert_eq!(cmd.params["copies"].as_array().unwrap().len(), 6);
    let makers = q.manufacturers();
    assert_eq!(makers.len(), 3);
    assert!(makers.iter().all(|(_, m)| m.library_id.as_uuid() == project));
    let mut names: Vec<String> = parts
        .iter()
        .map(|(_, p)| {
            assert!(p.item.manufacturer.extra.is_empty());
            ew_derive::manufacturer_name(&q, &p.item.manufacturer)
        })
        .collect();
    names.sort();
    assert_eq!(names, ["Generic Panels", "Phoenix Contact", "Schneider Electric"]);

    drop(q);
    // A second run: every part is held and unchanged, no command.
    let (again, cmd) = s
        .catalog_parts(&input, &loaded, &master, date("2026-10-29"))
        .unwrap();
    assert_eq!(again.counts().unchanged, 3, "{:?}", again.rows);
    assert!(cmd.is_none());
}

/// IM-6: into a writable library. The first import writes the parts and the new manufacturer; a second import of
/// the changed list keeps the item IDs, is refused for a part changed under its held version (nothing written)
/// and, bumped, writes it as the next patch version while the unchanged parts stay as they are.
#[test]
fn a_second_import_into_a_library_updates_in_place_and_needs_a_bump() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-catalog-library"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut reg = Registry::open(&tiers(&root.0)).unwrap();
    let lib = company_library(&mut reg, &root.0);
    let first = read_input(&write_file(&root.0, "supplier-a.csv", LIST), &Limits::default()).unwrap();
    let one = import_into_library(&mut reg, lib, &first, &map(), date("2026-10-29"), false, false).unwrap();
    assert_eq!(
        statuses(&one),
        [
            ("A9P52610".to_owned(), "new".to_owned()),
            ("3209510".to_owned(), "new".to_owned()),
            ("GP-100".to_owned(), "new".to_owned()),
            (String::new(), "rejected: no part number".to_owned()),
        ],
    );
    assert_eq!(one.import.manufacturers.len(), 1, "Generic Panels is new");
    let write = one.write.as_ref().expect("written");
    assert_eq!(write.added.len(), 4, "three parts and a manufacturer");
    let items: Vec<Uuid> = one.import.rows.iter().filter_map(|r| r.item).collect();
    assert_eq!(items.len(), 3);
    let breaker = items[0];
    assert_eq!(held(&reg, lib, breaker).0, "1.0.0");
    let before = held(&reg, lib, breaker);

    // The changed list under the same versions: refused as a whole, nothing written.
    let second = read_input(
        &write_file(&root.0, "supplier-a.csv", LIST_CHANGED),
        &Limits::default(),
    )
    .unwrap();
    match import_into_library(&mut reg, lib, &second, &map(), date("2027-10-29"), false, false) {
        Err(CatalogError::NeedsBump(rows)) => assert_eq!(rows, ["line 2: A9P52610 (1.0.0)"]),
        other => panic!("{other:?}"),
    }
    assert_eq!(held(&reg, lib, breaker), before, "nothing was written");

    // Bumped: the changed part is 1.0.1 under the same item ID; the new part is added; the rest is unchanged, and
    // Generic Panels now resolves through the library's master list.
    let two = import_into_library(&mut reg, lib, &second, &map(), date("2027-10-29"), true, false).unwrap();
    assert_eq!(
        statuses(&two),
        [
            ("A9P52610".to_owned(), "updated 1.0.0 -> 1.0.1".to_owned()),
            ("3209510".to_owned(), "unchanged".to_owned()),
            ("GP-100".to_owned(), "unchanged".to_owned()),
            ("GP-200".to_owned(), "new".to_owned()),
        ],
    );
    assert!(two.import.manufacturers.is_empty(), "no new manufacturer");
    let items2: Vec<Uuid> = two.import.rows.iter().filter_map(|r| r.item).collect();
    assert_eq!(&items2[..3], &items[..], "the same item IDs");
    let (version, description) = held(&reg, lib, breaker);
    assert_eq!(version, "1.0.1");
    assert_eq!(description, json!("Miniature circuit breaker"));
}

/// The library lock (doc 05 §7.4): a live lock refuses the import, `take_over` included; a stale lock refuses it
/// without `take_over` and is taken over with it.
#[test]
fn a_stale_library_lock_is_taken_over_only_when_asked() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-catalog-lock"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut reg = Registry::open(&tiers(&root.0)).unwrap();
    let lib = company_library(&mut reg, &root.0);
    let dir = root.0.join("company").join("std.ewlib");
    let other = Identity {
        host: "another-host".into(),
        ..Identity::current("other")
    };
    std::fs::write(
        dir.join(LOCK),
        other
            .record(std::time::UNIX_EPOCH + Duration::from_secs(1_800_000_000))
            .to_text(),
    )
    .unwrap();
    let input = read_input(&write_file(&root.0, "supplier-a.csv", LIST), &Limits::default()).unwrap();
    for take_over in [false, true] {
        match import_into_library(
            &mut reg,
            lib,
            &input,
            &map(),
            date("2026-10-29"),
            false,
            take_over,
        ) {
            Err(CatalogError::Library(LibraryError::Locked { host, .. })) => assert_eq!(host, "another-host"),
            other => panic!("a live lock refuses (take_over {take_over}): {other:?}"),
        }
    }
    // The lock goes stale: its file was last touched two hours ago.
    let file = std::fs::File::options().write(true).open(dir.join(LOCK)).unwrap();
    let written = file.metadata().unwrap().modified().unwrap();
    file.set_modified(written - Duration::from_secs(7200)).unwrap();
    drop(file);
    match import_into_library(&mut reg, lib, &input, &map(), date("2026-10-29"), false, false) {
        Err(CatalogError::Library(LibraryError::Stale { host, .. })) => assert_eq!(host, "another-host"),
        other => panic!("a stale lock needs the takeover: {other:?}"),
    }
    let done = import_into_library(&mut reg, lib, &input, &map(), date("2026-10-29"), false, true).unwrap();
    assert_eq!(done.write.expect("written").added.len(), 4);
    assert!(!dir.join(LOCK).exists(), "released after the write");
}

/// A library that is not installed, and one the user cannot write, are refused before the file is parsed.
#[test]
fn an_unknown_or_read_only_library_is_refused() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-catalog-refused"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut reg = Registry::open(&tiers(&root.0)).unwrap();
    let input = read_input(&write_file(&root.0, "supplier-a.csv", LIST), &Limits::default()).unwrap();
    let unknown = Uuid::from_u128(0x0190_a000_0000_7000_8000_0000_0000_0bad);
    assert!(matches!(
        import_into_library(&mut reg, unknown, &input, &map(), None, false, false),
        Err(CatalogError::Library(LibraryError::NotFound))
    ));
    let starter = ew_api::library::starter::LIBRARY;
    assert!(matches!(
        import_into_library(&mut reg, starter, &input, &map(), None, false, false),
        Err(CatalogError::Library(LibraryError::ReadOnly))
    ));
}

/// The library of the manufacturer record a held part names.
fn maker_library(reg: &Registry, library: Uuid, item: Uuid) -> Uuid {
    let r = LibRef {
        library: ew_ids::Id::from_uuid(library),
        item: ew_ids::Id::from_uuid(item),
        version_req: "*".into(),
        extra: Default::default(),
    };
    match reg.resolve(&r).unwrap() {
        LibDocument::Part(p) => p.item.manufacturer.library.as_uuid(),
        other => panic!("not a part: {:?}", other.kind()),
    }
}

/// FT-651: a library-target import names another library's manufacturer record only when the target's manifest
/// `depends` names that library. Without it, the import writes a copy of each manufacturer into the target and the
/// parts name the copies, so a second import finds the target's copy first and is not ambiguous; with it, the
/// parts name the depended-on library's records and only the new manufacturer is written.
#[test]
fn a_library_import_names_another_librarys_manufacturer_only_through_depends() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-catalog-ft651"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut reg = Registry::open(&tiers(&root.0)).unwrap();
    let std_lib = company_library(&mut reg, &root.0);
    let mine = reg
        .create(
            &root.0.join("company").join("mine.ewlib"),
            "mine",
            "Mine",
            "CC0-1.0",
        )
        .unwrap();
    let input = read_input(&write_file(&root.0, "supplier-a.csv", LIST), &Limits::default()).unwrap();
    let one = import_into_library(&mut reg, mine, &input, &map(), date("2026-10-29"), false, false).unwrap();
    let items: Vec<Uuid> = one.import.rows.iter().filter_map(|r| r.item).collect();
    assert_eq!(items.len(), 3);
    assert_eq!(
        one.import.manufacturers.len(),
        3,
        "Schneider Electric and Phoenix Contact copied, Generic Panels new"
    );
    for item in &items {
        assert_eq!(
            maker_library(&reg, mine, *item),
            mine,
            "the part names the target's copy"
        );
    }
    let again =
        import_into_library(&mut reg, mine, &input, &map(), date("2026-10-29"), false, false).unwrap();
    assert!(
        statuses(&again)
            .iter()
            .all(|(_, s)| s == "unchanged" || s.starts_with("rejected")),
        "{:?}",
        statuses(&again)
    );
    assert!(
        again.import.manufacturers.is_empty(),
        "the target's copies are found first"
    );

    // A library whose manifest depends on `std`: the parts name std's records.
    let dir = root.0.join("company").join("dep.ewlib");
    let dep = reg.create(&dir, "dep", "Dependent", "CC0-1.0").unwrap();
    let manifest = dir.join(ew_api::library::MANIFEST);
    let text = std::fs::read_to_string(&manifest).unwrap();
    let with = text.replace(
        "\"depends\": []",
        &format!("\"depends\": [{{\"library\":\"{std_lib}\",\"version_req\":\"^1\"}}]"),
    );
    assert_ne!(with, text, "the manifest names the dependency");
    std::fs::write(&manifest, with).unwrap();
    let mut reg = Registry::open(&tiers(&root.0)).unwrap();
    assert_eq!(reg.depends(dep), [std_lib]);
    let three = import_into_library(&mut reg, dep, &input, &map(), date("2026-10-29"), false, false).unwrap();
    assert_eq!(
        three.import.manufacturers.len(),
        1,
        "only Generic Panels is written"
    );
    let by_number: Vec<(String, Uuid)> = three
        .import
        .rows
        .iter()
        .filter_map(|r| Some((r.part_number.clone(), maker_library(&reg, dep, r.item?))))
        .collect();
    assert_eq!(
        by_number,
        [
            ("A9P52610".to_owned(), std_lib),
            ("3209510".to_owned(), std_lib),
            ("GP-100".to_owned(), dep),
        ]
    );
}
