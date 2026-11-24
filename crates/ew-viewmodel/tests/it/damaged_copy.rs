// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1383: a copy of the sample whose one library copy that a device names is damaged opens read-only in
//! the window, as a damaged copy no device names does, instead of being refused whole: every page shows and the
//! checks run.

use crate::common;

use common::{Tmp, copy_dir, done, failed};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// A headless shell in `locale` with its own configuration under `tmp`, `dir` open for editing.
fn open_in(tmp: &Tmp, locale: &str, dir: &std::path::Path) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join(format!("config-{locale}")), "0.0.0-test"),
    );
    assert!(strings::set_locale(locale));
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    s
}

/// A copy of the sample in `tmp` whose first part copy a device names is cut short: the folder and the copy's ID.
fn damaged_sample(tmp: &Tmp) -> (std::path::PathBuf, String) {
    let dir = tmp.sample_copy();
    copy_dir(std::path::Path::new(common::SAMPLE), &dir);
    // The first part copy a device names, cut short.
    let mut shards: Vec<_> = std::fs::read_dir(dir.join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    shards.sort();
    let copy = shards
        .iter()
        .find_map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    std::fs::write(
        dir.join("lib/parts").join(format!("{copy}.json")),
        "{\"$k\": \"part\",\n",
    )
    .unwrap();
    (dir, copy)
}

#[test]
fn a_damaged_copy_a_device_names_opens_read_only_and_every_page_shows() {
    let tmp = Tmp::new("ft1383");
    let (dir, _) = damaged_sample(&tmp);
    let mut s = tmp.empty_shell(Mode::Headless);
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    assert!(s.read_only_reason().is_some(), "the project opens read-only");
    done(&mut s, "page.first", json!({}));
    let mut seen = Vec::new();
    while let Some(folio) = s.current_folio().map(str::to_owned) {
        if seen.contains(&folio) {
            break;
        }
        assert!(s.canvas().is_some(), "page {folio} shows");
        seen.push(folio);
        let _ = s.dispatch("page.next", &json!({}));
    }
    assert!(seen.len() > 10, "{seen:?}");
    done(&mut s, "messages.check", json!({}));
}

/// The read-only banner names the device, the damaged copy in words and the way out, and Save As
/// is refused in those words before it asks for a folder: a copy without the damaged file would not open. Nothing is
/// written. Both languages.
#[test]
fn save_as_is_refused_and_the_banner_names_the_device_and_the_copy() {
    let tmp = Tmp::new("ft1383-save-as");
    let (dir, _) = damaged_sample(&tmp);
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the device -201K1-Q1 uses the library copy of part EW-GEN-VLV-3 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first. 1 integrity \
             finding was found when the project was opened.",
            "Save as is refused: the device -201K1-Q1 uses the library copy of part EW-GEN-VLV-3 from ElektraWatt \
             starter parts, which is damaged, and a copy of the project without it would not open. Update the copy \
             from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat -201K1-Q1 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-VLV-3 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: het apparaat -201K1-Q1 gebruikt de bibliotheekkopie van artikel EW-GEN-VLV-3 \
             uit ElektraWatt starter parts, die beschadigd is, en een projectkopie zonder deze bibliotheekkopie zou \
             niet openen. Werk de kopie eerst bij vanuit de bibliotheek.",
        ),
    ] {
        let mut s = open_in(&tmp, locale, &dir);
        assert_eq!(s.banner(), Some(banner));
        assert_eq!(
            failed(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()})),
            save_as
        );
        assert!(!target.exists(), "nothing is written");
        assert!(
            s.read_only_reason().is_some(),
            "the session stays on the read-only project"
        );
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// The damaged copy's banner stays while the project is read-only for it: after a page change it
/// still names the device and the way out (the open's finding count goes with the message), and it cannot be
/// dismissed; it follows the UI language, and it goes when the project closes.
#[test]
fn the_damaged_copy_banner_stays_while_its_state_holds() {
    let tmp = Tmp::new("id1427-damaged-banner");
    let (dir, _) = damaged_sample(&tmp);
    let mut s = tmp.empty_shell(Mode::Headless);
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut s, "page.next", json!({}));
    assert_eq!(
        s.banner(),
        Some(
            "This project is open read-only: the device -201K1-Q1 uses the library copy of part EW-GEN-VLV-3 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first."
        )
    );
    assert!(!s.banner_dismissable());
    done(&mut s, "view.language", json!({"value": "nl"}));
    assert_eq!(
        s.banner(),
        Some(
            "Dit project is alleen-lezen geopend: het apparaat -201K1-Q1 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-VLV-3 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek."
        )
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    done(&mut s, "file.close", json!({}));
    assert_eq!(s.banner(), None);
    assert!(strings::set_locale("en"));
}

/// A copy of the sample in `tmp` whose library copy `file` (under `lib/`) is cut short: the folder.
fn sample_with_damaged(tmp: &Tmp, file: &str) -> std::path::PathBuf {
    let dir = tmp.sample_copy();
    copy_dir(std::path::Path::new(common::SAMPLE), &dir);
    std::fs::write(dir.join("lib").join(file), "{\"$k\": \"form\",\n").unwrap();
    dir
}

/// The files of a project folder and their bytes, for "nothing is written".
fn tree(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// A damaged library copy that no device names, here the frame form the report blocks and the panel-layout page
/// name, is named in the banner and the Save As refusal by its kind and by the first record that names it, a report
/// block, in both languages; nothing is written.
#[test]
fn a_damaged_copy_a_record_names_is_named_with_that_record() {
    let tmp = Tmp::new("ft1500-form");
    let dir = sample_with_damaged(&tmp, "forms/ec586fad-385b-599d-a76d-59c893506f00.json");
    let before = tree(&dir);
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the library copy of a form named by a report block is damaged. No library on \
             this computer holds it, so the copy cannot be updated from one. 1 integrity finding was found when the \
             project was opened.",
            "Save as is refused: the library copy of a form named by a report block is damaged, and a copy of the project \
             without it would not open. Update the copy from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een formulier waarnaar een rapportblok verwijst \
             is beschadigd. Geen bibliotheek op deze computer bevat het, dus de kopie kan niet vanuit een \
             bibliotheek worden bijgewerkt. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: de bibliotheekkopie van een formulier waarnaar een rapportblok verwijst is \
             beschadigd, en een projectkopie zonder deze bibliotheekkopie zou niet openen. Werk de kopie eerst bij \
             vanuit de bibliotheek.",
        ),
    ] {
        let mut s = open_in(&tmp, locale, &dir);
        assert_eq!(s.banner(), Some(banner));
        assert_eq!(
            failed(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()})),
            save_as
        );
        assert!(!target.exists(), "nothing is written");
        assert!(s.read_only_reason().is_some());
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written inside the project");
}

/// A damaged library copy that no record names (the cover frame, which only the project's settings
/// name) leaves the project read-only with a banner that names it and the way out; Save As then makes a copy of
/// the project without it, which opens for editing, and its toast names the copy left out; the banner goes. The
/// damaged project is never written.
#[test]
fn save_as_leaves_out_a_damaged_copy_no_record_names() {
    let tmp = Tmp::new("ft1500-left-out");
    let file = "forms/81ee0d6d-6c0e-55e4-b7fd-1896f594051f.json";
    let dir = sample_with_damaged(&tmp, file);
    let before = tree(&dir);
    for (locale, banner, toast) in [
        (
            "en",
            "This project is open read-only: the library copy of a form is damaged. No library on this \
             computer holds it; use Save as for a copy of the project without it. 1 integrity finding was found when \
             the project was opened.",
            "Saved as save-as-en without the library copy of a form, which is damaged.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een formulier is beschadigd. Geen \
             bibliotheek op deze computer bevat het; maak met Opslaan als een projectkopie zonder deze \
             bibliotheekkopie. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opgeslagen als save-as-nl zonder de bibliotheekkopie van een formulier, die beschadigd is.",
        ),
    ] {
        let target = tmp.0.join(format!("save-as-{locale}"));
        let mut s = open_in(&tmp, locale, &dir);
        assert_eq!(s.banner(), Some(banner));
        assert!(s.read_only_reason().is_some());
        done(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()}));
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(toast));
        assert!(s.read_only_reason().is_none(), "the copy edits");
        assert_eq!(s.banner(), None);
        assert!(
            !target.join("lib").join(file).exists(),
            "the damaged copy is left out"
        );
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
    assert!(tree(&dir) == before, "nothing is written inside the project");
}

/// Two damaged copies that no record names are both left out of a Save As, and the banner and the
/// toast name both, whole in both languages.
#[test]
fn every_damaged_copy_a_save_as_leaves_out_is_named() {
    let tmp = Tmp::new("left-out-two");
    let dir = sample_with_damaged(&tmp, "forms/81ee0d6d-6c0e-55e4-b7fd-1896f594051f.json");
    std::fs::write(
        dir.join("lib/forms/c80af135-34f8-574c-8f52-756bd24c7bd0.json"),
        "{\"$k\": \"form\",\n",
    )
    .unwrap();
    for (locale, banner, toast) in [
        (
            "en",
            "This project is open read-only: the library copy of a form, the library copy of a form \
             are damaged. No library on this computer holds them; use Save as for a copy of the project without \
             them. 2 integrity findings were found when the project was opened.",
            "Saved as save-as-en without the library copy of a form, the library copy of a form, \
             which are damaged.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een formulier, de bibliotheekkopie van een \
             formulier zijn beschadigd. Geen bibliotheek op deze computer bevat ze; maak met Opslaan als een \
             projectkopie zonder deze bibliotheekkopieën. Bij het openen van het project zijn 2 \
             integriteitsbevindingen gevonden.",
            "Opgeslagen als save-as-nl zonder de bibliotheekkopie van een formulier, de bibliotheekkopie van een \
             formulier, die beschadigd zijn.",
        ),
    ] {
        let target = tmp.0.join(format!("save-as-{locale}"));
        let mut s = open_in(&tmp, locale, &dir);
        assert_eq!(s.banner(), Some(banner));
        done(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()}));
        assert_eq!(s.toast().map(|t| t.text.as_str()), Some(toast));
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// FT-1383: a damaged definition copy whose functions end fluid lines (the sample's pneumatic cylinders on
/// folio 201, EW-INV-024 beside the EW-INV-001 that names the copy) opens read-only with FT-1383's banner, where the
/// open was refused before; Save As is refused in its words as for any copy a record names, nothing being
/// written. Both languages.
#[test]
fn a_damaged_definition_copy_with_fluid_lines_opens_read_only() {
    let tmp = Tmp::new("id1484-fluid");
    let dir = sample_with_damaged(&tmp, "fndefs/7b85e9bb-116f-5c3c-a7b6-d5260aa1c219.json");
    let before = tree(&dir);
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the device -201M1 uses the library copy of its part, which is \
             damaged. Update the copy from its library first. 1 integrity finding was found when the project was \
             opened.",
            "Save as is refused: the device -201M1 uses the library copy of its part, which is damaged, and a copy \
             of the project without it would not open. Update the copy from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat -201M1 gebruikt de bibliotheekkopie van zijn \
             artikel, die beschadigd is. Werk de kopie eerst bij vanuit de bibliotheek. Bij het openen van het \
             project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: het apparaat -201M1 gebruikt de bibliotheekkopie van zijn artikel, die \
             beschadigd is, en een projectkopie zonder deze bibliotheekkopie zou niet openen. Werk de kopie eerst \
             bij vanuit de bibliotheek.",
        ),
    ] {
        let mut s = open_in(&tmp, locale, &dir);
        assert!(s.read_only_reason().is_some(), "the project opens read-only");
        assert_eq!(s.banner(), Some(banner));
        assert_eq!(
            failed(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()})),
            save_as
        );
        assert!(!target.exists(), "nothing is written");
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written inside the project");
}

/// A damaged copy of a part an installed library holds (the compiled-in starter parts, assigned to
/// -101F1 and saved) is named in the banner and the Save As refusal by its part number and its library, whole in both
/// languages.
#[test]
fn a_damaged_copy_of_an_installed_part_is_named_whole() {
    let tmp = Tmp::new("ft1525");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "select.set", json!({ "refs": ["tag:-101F1"] }));
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
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
    done(&mut s, "file.save", json!({}));
    done(&mut s, "file.close", json!({}));
    let dir = tmp.sample_copy();
    std::fs::write(
        dir.join("lib/parts").join(format!("{copy}.json")),
        "{\"$k\": \"part\",\n",
    )
    .unwrap();
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the device -101F1 uses the library copy of part EW-GEN-MCB-2 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first. 1 integrity \
             finding was found when the project was opened.",
            "Save as is refused: the device -101F1 uses the library copy of part EW-GEN-MCB-2 from ElektraWatt \
             starter parts, which is damaged, and a copy of the project without it would not open. Update the copy \
             from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat -101F1 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-MCB-2 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: het apparaat -101F1 gebruikt de bibliotheekkopie van artikel EW-GEN-MCB-2 uit \
             ElektraWatt starter parts, die beschadigd is, en een projectkopie zonder deze bibliotheekkopie zou \
             niet openen. Werk de kopie eerst bij vanuit de bibliotheek.",
        ),
    ] {
        let mut s = open_in(&tmp, locale, &dir);
        assert_eq!(s.banner(), Some(banner));
        assert_eq!(
            failed(&mut s, "file.save_as", json!({"dir": target.to_string_lossy()})),
            save_as
        );
        assert!(!target.exists(), "nothing is written");
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}
