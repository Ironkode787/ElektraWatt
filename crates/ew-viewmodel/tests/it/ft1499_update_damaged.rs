// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1499 (ID-1486): the read-only banner of a project with a damaged library copy (FT-1383) offers Update from library
//! when an installed library holds the copy's item; the action writes the library's copy over the damaged file and
//! opens the project again, writable when the integrity check passes. Several damaged copies: every one a library holds
//! is updated and the others are named. No library holds it: the banner says so and offers nothing. Nothing is written
//! when the library's copy cannot be read or another session holds the project.

use crate::common;

use std::path::{Path, PathBuf};

use common::{Tmp, copy_dir, done, refused};
use ew_viewmodel::ew_api::library::{LibraryTiers, Registry};
use ew_viewmodel::ew_api::{OpenMode, Recovery, Session, SessionOptions, StoreOptions};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::{key, strings};
use serde_json::{Value, json};

/// Assigns the part `number` of the shell's libraries to the device F55 of the open project, saves and closes it: the
/// part copy's ID.
fn assign_f55(s: &mut ShellModel, number: &str) -> String {
    done(s, "select.set", json!({ "refs": ["tag:F55"] }));
    done(s, "part.choose", json!({ "use": 0 }));
    done(s, "part.pick", json!({ "part": number }));
    let params = s
        .recorded_commands()
        .last()
        .expect("the assignment")
        .params
        .clone();
    let copy = params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .find(|c| c["$k"] == "part")
        .expect("the part's copy")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    done(s, "file.save", json!({}));
    done(s, "file.close", json!({}));
    copy
}

/// A copy of Golden A in `tmp` whose F55 uses the compiled-in starter part EW-GEN-MCB-2: the folder, the part copy's
/// file and its bytes as the save wrote them from the library.
fn starter_project(tmp: &Tmp) -> (PathBuf, PathBuf, Vec<u8>) {
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let copy = assign_f55(&mut s, "EW-GEN-MCB-2");
    let dir = tmp.golden_copy();
    let file = dir.join("lib/parts").join(format!("{copy}.json"));
    let bytes = std::fs::read(&file).unwrap();
    (dir, file, bytes)
}

/// Cuts the library copy `file` short.
fn damage(file: &Path) {
    std::fs::write(file, "{\"$k\": \"part\",\n").unwrap();
}

/// A headless shell in `locale` with its own configuration `config-<locale>` under `tmp` and the libraries
/// `libraries`, `dir` open for editing.
fn open(tmp: &Tmp, locale: &str, libraries: Option<LibraryTiers>, dir: &Path) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join(format!("config-{locale}")), "0.0.0-test").with_libraries(libraries),
    );
    assert!(strings::set_locale(locale));
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    s
}

/// The files of a project folder and their bytes, but the lock, for "nothing is written".
fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else if p.file_name().is_some_and(|n| n != ".~ew.lock") {
                out.push((p.clone(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// The round trip: the banner of a damaged copy of a starter part offers Update from library; the action writes the
/// library's copy over the damaged file, byte for byte as the assignment's save wrote it, and the project opens for
/// editing with no banner; the toast names the copy with the version written (C53-p4#4). Both languages.
#[test]
fn ft1499_update_from_library_writes_the_copy_and_opens_the_project_for_editing() {
    let tmp = Tmp::new("ft1499-round-trip");
    let (dir, file, original) = starter_project(&tmp);
    for (locale, banner, toast) in [
        (
            "en",
            "This project is open read-only: the device F55 uses the library copy of part EW-GEN-MCB-2 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first. 1 integrity \
             finding was found when the project was opened.",
            "Updated from library: the library copy of part EW-GEN-MCB-2 at version 0.1.1.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat F55 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-MCB-2 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Bijgewerkt vanuit bibliotheek: de bibliotheekkopie van artikel EW-GEN-MCB-2 in versie 0.1.1.",
        ),
    ] {
        damage(&file);
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.read_only_reason().is_some(), "the project opens read-only");
        assert_eq!(s.banner(), Some(banner));
        assert!(s.damaged_update_offered(), "the banner offers the update");
        assert_eq!(
            strings::text(key!("action-library-update-damaged")),
            if locale == "en" {
                "Update from library"
            } else {
                "Bijwerken vanuit bibliotheek"
            }
        );
        done(&mut s, "library.update_damaged", json!({}));
        assert!(s.read_only_reason().is_none(), "the project edits");
        assert_eq!(s.banner(), None);
        assert!(!s.damaged_update_offered());
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(toast));
        assert!(
            std::fs::read(&file).unwrap() == original,
            "the library's copy is written"
        );
        // The device's part is the library's: Save As copies the project, which opens for editing.
        let target = tmp.0.join(format!("save-as-{locale}"));
        done(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()}));
        done(&mut s, "file.close", json!({}));
        done(
            &mut s,
            "file.open",
            json!({"dir": target.to_string_lossy(), "read_only": false}),
        );
        assert!(s.read_only_reason().is_none(), "the copy opens for editing");
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// Golden A's first part copy a device names (the device -201U8-Q1's), which no library holds: its file in `dir`.
fn golden_part_copy(dir: &Path) -> PathBuf {
    let mut shards: Vec<_> = std::fs::read_dir(dir.join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    shards.sort();
    let golden = shards
        .iter()
        .find_map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    dir.join("lib/parts").join(format!("{golden}.json"))
}

/// The manifest's entry of the project file `path` (relative, with `/`) in `dir`.
fn manifest_entry(dir: &Path, path: &str) -> Value {
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    manifest["files"][path].clone()
}

/// C53-p4#5: the banner offers the update only for the copy it names. It names Golden A's part copy of -201U8-Q1,
/// which no library holds, so it says so and offers nothing, though a library holds the starter part's damaged copy
/// too; nothing is written. Both languages.
#[test]
fn ft1499_the_banner_offers_the_update_only_for_the_copy_it_names() {
    let tmp = Tmp::new("ft1499-named");
    let (dir, file, _) = starter_project(&tmp);
    damage(&golden_part_copy(&dir));
    damage(&file);
    let before = tree(&dir);
    for (locale, banner) in [
        (
            "en",
            "This project is open read-only: the device -201U8-Q1 uses the library copy of its part, which is \
             damaged. No library on this computer holds it, so the copy cannot be updated from one. 2 integrity \
             findings were found when the project was opened.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat -201U8-Q1 gebruikt de bibliotheekkopie van zijn \
             artikel, die beschadigd is. Geen bibliotheek op deze computer bevat het, dus de kopie kan niet vanuit \
             een bibliotheek worden bijgewerkt. Bij het openen van het project zijn 2 integriteitsbevindingen \
             gevonden.",
        ),
    ] {
        let mut s = open(&tmp, locale, None, &dir);
        assert_eq!(s.banner(), Some(banner));
        assert!(!s.damaged_update_offered(), "nothing is offered");
        assert_eq!(
            refused(&mut s, "library.update_damaged", json!({})),
            "reason-no-damaged-copy-update"
        );
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// Several damaged copies (C53-p4#1): the starter part's, which a library holds and the banner names, is updated, and a
/// manufacturer copy of Golden A that no record names and no library holds, whose file is gone, stays damaged with its
/// manifest entry and is named in the toast; the project opens read-only again with that copy's banner, which offers
/// nothing. Both languages.
#[test]
fn ft1499_several_copies_update_every_one_a_library_holds_and_name_the_rest() {
    let tmp = Tmp::new("ft1499-several");
    let (dir, file, original) = starter_project(&tmp);
    let gone = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let entry = manifest_entry(&dir, gone);
    assert!(entry.is_string(), "{gone} is listed");
    std::fs::remove_file(dir.join(gone)).unwrap();
    for (locale, before, toast, after) in [
        (
            "en",
            "This project is open read-only: the device F55 uses the library copy of part EW-GEN-MCB-2 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first. 2 integrity \
             findings were found when the project was opened.",
            "Updated from library: the library copy of part EW-GEN-MCB-2 at version 0.1.1. Not updated, as no \
             library on this computer holds a readable version: the library copy of a manufacturer.",
            "This project is open read-only: the library copy of a manufacturer is damaged. No library on this \
             computer holds it; use Save as for a copy of the project without it. 1 integrity finding was found when \
             the project was opened.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat F55 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-MCB-2 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek. Bij het openen van het project zijn 2 integriteitsbevindingen gevonden.",
            "Bijgewerkt vanuit bibliotheek: de bibliotheekkopie van artikel EW-GEN-MCB-2 in versie 0.1.1. Niet \
             bijgewerkt, omdat geen bibliotheek op deze computer een leesbare versie heeft: de bibliotheekkopie van \
             een fabrikant.",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een fabrikant is beschadigd. Geen \
             bibliotheek op deze computer bevat het; maak met Opslaan als een projectkopie zonder deze \
             bibliotheekkopie. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
        ),
    ] {
        damage(&file);
        let mut s = open(&tmp, locale, None, &dir);
        assert_eq!(s.banner(), Some(before));
        assert!(s.damaged_update_offered());
        done(&mut s, "library.update_damaged", json!({}));
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(toast));
        assert!(
            std::fs::read(&file).unwrap() == original,
            "the starter part's copy is written"
        );
        assert_eq!(manifest_entry(&dir, gone), entry, "the entry of the copy left damaged");
        assert!(s.read_only_reason().is_some(), "the project stays read-only");
        assert_eq!(s.banner(), Some(after));
        assert!(!s.damaged_update_offered());
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// Two damaged copies a library holds, the starter part's and its manufacturer's (C53-p2's plural toast, C53-p4#2 and
/// #4): both are written and the toast names each with the version written. Both languages.
#[test]
fn ft1499_every_copy_written_is_named_with_its_version() {
    let tmp = Tmp::new("ft1499-two");
    let (dir, file, original) = starter_project(&tmp);
    let maker = dir.join("lib/manufacturers/7ae048b6-63ea-5c01-83b2-e1a83b5b4414.json");
    let maker_bytes = std::fs::read(&maker).unwrap();
    for (locale, toast) in [
        (
            "en",
            "Updated from library: the library copy of part EW-GEN-MCB-2 at version 0.1.1, the library copy of \
             manufacturer Generic (Generic) at version 0.1.0.",
        ),
        (
            "nl",
            "Bijgewerkt vanuit bibliotheek: de bibliotheekkopie van artikel EW-GEN-MCB-2 in versie 0.1.1, de \
             bibliotheekkopie van fabrikant Generic (Generic) in versie 0.1.0.",
        ),
    ] {
        damage(&file);
        std::fs::write(&maker, "{\"$k\": \"manufacturer\",\n").unwrap();
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.damaged_update_offered());
        done(&mut s, "library.update_damaged", json!({}));
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(toast));
        assert!(s.read_only_reason().is_none(), "the project edits");
        assert!(std::fs::read(&file).unwrap() == original);
        assert!(std::fs::read(&maker).unwrap() == maker_bytes);
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// Another file of the project changed outside ElektraWatt (a page file, EW-CORE-002) beside the damaged copy: the
/// update is refused in words, whole in both languages, and nothing is written (C53-p4#2).
#[test]
fn ft1499_other_integrity_findings_refuse_the_update() {
    let tmp = Tmp::new("ft1499-findings");
    let (dir, file, _) = starter_project(&tmp);
    damage(&file);
    let mut pages: Vec<_> = std::fs::read_dir(dir.join("pages"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    pages.sort();
    let mut edited = std::fs::read(&pages[0]).unwrap();
    edited.extend_from_slice(b"\n");
    std::fs::write(&pages[0], edited).unwrap();
    let before = tree(&dir);
    for (locale, message) in [
        (
            "en",
            "Update from library is refused: the project has other integrity findings, which keep it read-only; \
             nothing was written.",
        ),
        (
            "nl",
            "Bijwerken vanuit de bibliotheek is geweigerd: het project heeft andere integriteitsbevindingen, die het \
             alleen-lezen houden; er is niets geschreven.",
        ),
    ] {
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.damaged_update_offered());
        match s.dispatch("library.update_damaged", &json!({})) {
            Outcome::Failed { message: m } => assert_eq!(m, message),
            other => panic!("{other:?}"),
        }
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// The save fails (the manifest changed on disk after the open): the update is refused in words, whole in both
/// languages, and nothing more is written (C53-p4#2).
#[test]
fn ft1499_a_save_that_fails_refuses_the_update() {
    let tmp = Tmp::new("ft1499-save");
    let (dir, file, _) = starter_project(&tmp);
    let manifest = dir.join("manifest.json");
    let original = std::fs::read(&manifest).unwrap();
    for (locale, message) in [
        (
            "en",
            "Update from library is refused: the update could not be written: the project changed on disk \
             (manifest.json); merge the change first.",
        ),
        (
            "nl",
            "Bijwerken vanuit de bibliotheek is geweigerd: het bijwerken kon niet worden weggeschreven: the project \
             changed on disk (manifest.json); merge the change first.",
        ),
    ] {
        damage(&file);
        std::fs::write(&manifest, &original).unwrap();
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.damaged_update_offered());
        let mut changed = original.clone();
        changed.extend_from_slice(b"\n");
        std::fs::write(&manifest, &changed).unwrap();
        let before = tree(&dir);
        match s.dispatch("library.update_damaged", &json!({})) {
            Outcome::Failed { message: m } => assert_eq!(m, message),
            other => panic!("{other:?}"),
        }
        assert!(tree(&dir) == before, "nothing is written");
        done(&mut s, "file.close", json!({}));
    }
    std::fs::write(&manifest, &original).unwrap();
    assert!(strings::set_locale("en"));
}

/// A project opened read-only on request (C53-p4#6): the window shows no damaged-copy banner there, so it offers no
/// update and refuses the action with its reason; the session's own refusal is the read-only one. Each text whole in
/// both languages; nothing is written.
#[test]
fn ft1499_a_project_opened_read_only_on_request_offers_no_update() {
    let tmp = Tmp::new("ft1499-requested");
    let (dir, file, _) = starter_project(&tmp);
    damage(&file);
    let before = tree(&dir);
    for (locale, reason, refusal) in [
        (
            "en",
            "The open project has no damaged library copy that a library holds",
            "the project can only be opened read-only, for a reason other than its damaged library copies; nothing \
             was written",
        ),
        (
            "nl",
            "Het open project heeft geen beschadigde bibliotheekkopie die een bibliotheek bevat",
            "het project kan alleen als alleen-lezen worden geopend, om een andere reden dan zijn beschadigde \
             bibliotheekkopieën; er is niets geschreven",
        ),
    ] {
        let mut s = ShellModel::new(
            Mode::Headless,
            ShellConfig::in_dir(&tmp.0.join(format!("config-{locale}")), "0.0.0-test"),
        );
        assert!(strings::set_locale(locale));
        done(
            &mut s,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": true}),
        );
        assert!(s.read_only_reason().is_some());
        assert!(!s.damaged_update_offered(), "nothing is offered");
        assert_eq!(
            refused(&mut s, "library.update_damaged", json!({})),
            "reason-no-damaged-copy-update"
        );
        assert_eq!(strings::text(key!("reason-no-damaged-copy-update")), reason);
        done(&mut s, "file.close", json!({}));
        // The session's own refusal, for a front-end that asks it anyway.
        let mut store = StoreOptions::new("ElektraWatt test");
        store.state_dir = None;
        store.mode = OpenMode::ReadOnly;
        let (mut session, _) =
            Session::open(&dir, store, SessionOptions::default(), Recovery::Keep).expect("opens read-only");
        let text = session.update_damaged_copies().expect_err("refused");
        assert_eq!(text.key(), "refusal-update-copies-read-only");
        assert_eq!(strings::text(key!("refusal-update-copies-read-only")), refusal);
        session.close();
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// A library's version of the damaged copy that would break the project (C53-p4#3). Golden A's double-acting cylinder
/// -203Q1 gets a wire between its two ports, which its definition, damaged and dropped, does not judge; the starter
/// library's version makes them fluid ports, which no wire may end at (EW-INV-024). The update is refused with the
/// misfit's words, whole in both languages, and nothing is written.
#[test]
fn ft1499_a_library_version_that_breaks_the_project_refuses_the_update() {
    let tmp = Tmp::new("ft1499-misfit");
    let dir = tmp.golden_copy();
    copy_dir(Path::new(common::GOLDEN_A), &dir);
    let page = dir.join("pages/ce20ff4a-7c7c-587a-aeb9-b6ab6624ea08.jsonl");
    let cylinder = "eb6810a7-f538-5968-ac8c-cd35cc1c0725";
    let mut text = std::fs::read_to_string(&page).unwrap();
    assert!(text.contains(cylinder), "-203Q1's function is drawn on the page");
    text.push_str(&format!(
        "{{\"$k\":\"connection\",\"id\":\"9d1c53a4-0000-4000-8000-000000000001\",\"a\":{{\"cp\":[\"{cylinder}\",0]}},\
         \"b\":{{\"cp\":[\"{cylinder}\",1]}},\"page\":\"ce20ff4a-7c7c-587a-aeb9-b6ab6624ea08\"}}\n"
    ));
    std::fs::write(&page, text).unwrap();
    std::fs::write(
        dir.join("lib/fndefs/7b85e9bb-116f-5c3c-a7b6-d5260aa1c219.json"),
        "{\"$k\": \"fndef\",\n",
    )
    .unwrap();
    let before = tree(&dir);
    for (locale, message) in [
        (
            "en",
            "Update from library is refused: the libraries' versions of the damaged copies do not fit the project, \
             which would then not open; nothing was written.",
        ),
        (
            "nl",
            "Bijwerken vanuit de bibliotheek is geweigerd: de versies van de beschadigde kopieën in de bibliotheken \
             passen niet bij het project, dat dan niet zou openen; er is niets geschreven.",
        ),
    ] {
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.read_only_reason().is_some());
        assert!(s.damaged_update_offered(), "the banner offers the update");
        match s.dispatch("library.update_damaged", &json!({})) {
            Outcome::Failed { message: m } => assert_eq!(m, message),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.banner(), Some(message));
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// No library holds the damaged copy (Golden A's own part copy of -201U8-Q1): the banner says so (FT-1383's tests
/// assert it whole) and offers nothing; the action is refused with its reason, whole in both languages, and nothing is
/// written.
#[test]
fn ft1499_a_copy_no_library_holds_offers_nothing() {
    let tmp = Tmp::new("ft1499-no-library");
    let dir = tmp.golden_copy();
    copy_dir(Path::new(common::GOLDEN_A), &dir);
    let mut shards: Vec<_> = std::fs::read_dir(dir.join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    shards.sort();
    let golden = shards
        .iter()
        .find_map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    damage(&dir.join("lib/parts").join(format!("{golden}.json")));
    let before = tree(&dir);
    for (locale, reason) in [
        (
            "en",
            "The open project has no damaged library copy that a library holds",
        ),
        (
            "nl",
            "Het open project heeft geen beschadigde bibliotheekkopie die een bibliotheek bevat",
        ),
    ] {
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.read_only_reason().is_some());
        assert!(!s.damaged_update_offered(), "nothing is offered");
        let key = refused(&mut s, "library.update_damaged", json!({}));
        assert_eq!(key, "reason-no-damaged-copy-update");
        assert_eq!(strings::text(key!("reason-no-damaged-copy-update")), reason);
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// The tiers of a fixture under `root`: the compiled-in libraries and a user directory.
fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The library's copy cannot be read when the action runs (its file in the user library "Mine" cut short after the
/// banner offered it): the action is refused in words, whole in both languages, and nothing of the project is written.
#[test]
fn ft1499_nothing_is_written_when_the_library_copy_cannot_be_read() {
    let tmp = Tmp::new("ft1499-unreadable");
    let (dir, file, _) = starter_project(&tmp);
    // The starter part as the project holds it, as a part of the user library "Mine" with its own number.
    let t = tiers(&tmp.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib_dir = t.user.join("mine.ewlib");
    let lib = reg.create(&lib_dir, "mine", "Mine", "CC0-1.0").unwrap();
    let mut value: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value["library_id"] = json!(lib.hyphenated().to_string());
    value["part_number"] = json!("EW-TEST-MCB-2");
    value["meta"]["key"] = json!("EW-TEST-MCB-2");
    reg.write(lib, &[serde_json::from_value(value).unwrap()]).unwrap();
    drop(reg);
    let mut s = open(&tmp, "en", Some(t.clone()), &dir);
    let copy = assign_f55(&mut s, "EW-TEST-MCB-2");
    let file = dir.join("lib/parts").join(format!("{copy}.json"));
    damage(&file);
    let before = tree(&dir);
    for (locale, message) in [
        (
            "en",
            "Update from library is refused: no library on this computer holds a readable version of the library \
             copy of a part; nothing was written.",
        ),
        (
            "nl",
            "Bijwerken vanuit de bibliotheek is geweigerd: geen bibliotheek op deze computer heeft een leesbare \
             versie van de bibliotheekkopie van een artikel; er is niets geschreven.",
        ),
    ] {
        // The library's file is read again: it is whole when the project opens.
        let parts = lib_dir.join("parts");
        let saved: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&parts)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let b = std::fs::read(&p).unwrap();
                (p, b)
            })
            .collect();
        let mut s = open(&tmp, locale, Some(t.clone()), &dir);
        assert!(s.damaged_update_offered(), "the banner offers the update");
        for (p, _) in &saved {
            std::fs::write(p, "{\"$k\": \"part\",\n").unwrap();
        }
        match s.dispatch("library.update_damaged", &json!({})) {
            Outcome::Failed { message: m } => assert_eq!(m, message),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.banner(), Some(message));
        assert!(s.read_only_reason().is_some());
        done(&mut s, "file.close", json!({}));
        for (p, b) in &saved {
            std::fs::write(p, b).unwrap();
        }
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}

/// Another session holds the project when the action runs (its lock taken after the read-only open gave it back): the
/// action is refused in words, whole in both languages, and nothing is written.
#[test]
fn ft1499_update_is_refused_while_another_session_holds_the_project() {
    let tmp = Tmp::new("ft1499-locked");
    let (dir, file, _) = starter_project(&tmp);
    damage(&file);
    let before = tree(&dir);
    // Another session's lock: a live session of another copy holds its own, which is copied in.
    let other = tmp.0.join("other.ew");
    copy_dir(Path::new(common::GOLDEN_A), &other);
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = None;
    let (holder, _) = Session::open(&other, store, SessionOptions::default(), Recovery::Keep)
        .expect("the other copy opens");
    for (locale, message) in [
        (
            "en",
            "Update from library is refused: another session holds the project; nothing was written.",
        ),
        (
            "nl",
            "Bijwerken vanuit de bibliotheek is geweigerd: een andere sessie heeft het project open; er is niets \
             geschreven.",
        ),
    ] {
        let mut s = open(&tmp, locale, None, &dir);
        assert!(s.damaged_update_offered());
        std::fs::copy(other.join(".~ew.lock"), dir.join(".~ew.lock")).unwrap();
        match s.dispatch("library.update_damaged", &json!({})) {
            Outcome::Failed { message: m } => assert_eq!(m, message),
            other => panic!("{other:?}"),
        }
        assert_eq!(s.banner(), Some(message));
        done(&mut s, "file.close", json!({}));
        std::fs::remove_file(dir.join(".~ew.lock")).unwrap();
    }
    holder.close();
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written");
}
