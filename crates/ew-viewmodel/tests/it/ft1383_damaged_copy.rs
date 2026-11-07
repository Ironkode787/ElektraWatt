// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1383 (ID-1395): a copy of Golden A whose one library copy that a device names is damaged opens read-only in
//! the window, as a damaged copy no device names does, instead of being refused whole: every page shows and the
//! checks run.

use crate::common;

use common::{Tmp, copy_dir, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// A copy of Golden A in `tmp` whose first part copy a device names is cut short: the folder and the copy's ID.
fn damaged_golden(tmp: &Tmp) -> (std::path::PathBuf, String) {
    let dir = tmp.golden_copy();
    copy_dir(std::path::Path::new(common::GOLDEN_A), &dir);
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
fn ft1383_a_damaged_copy_a_device_names_opens_read_only_and_every_page_shows() {
    let tmp = Tmp::new("ft1383");
    let (dir, _) = damaged_golden(&tmp);
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
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

/// C52-p4#1, #2 (ID-1418; FT-1396): the read-only banner names the device, the damaged copy in words and the way out, and Save As
/// is refused in those words before it asks for a folder: a copy without the damaged file would not open. Nothing is
/// written. Both languages.
#[test]
fn ft1383_save_as_is_refused_and_the_banner_names_the_device_and_the_copy() {
    let tmp = Tmp::new("ft1383-save-as");
    let (dir, _) = damaged_golden(&tmp);
    let target = tmp.0.join("save-as");
    let refused =
        |s: &mut ShellModel| match s.dispatch("file.save_as", &json!({"dir": target.to_string_lossy()})) {
            Outcome::Failed { message } => message,
            other => panic!("{other:?}"),
        };
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the device -201U8-Q1 uses the library copy of its part, which is \
             damaged. Update the copy from its library first. 1 integrity finding was found when the project was \
             opened.",
            "Save as is refused: the device -201U8-Q1 uses the library copy of its part, which is damaged, and a copy \
             of the project without it would not open. Update the copy from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat -201U8-Q1 gebruikt de bibliotheekkopie van zijn \
             artikel, die beschadigd is. Werk de kopie eerst bij vanuit de bibliotheek. Bij het openen van het \
             project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: het apparaat -201U8-Q1 gebruikt de bibliotheekkopie van zijn artikel, die \
             beschadigd is, en een projectkopie zonder deze bibliotheekkopie zou niet openen. Werk de kopie eerst \
             bij vanuit de bibliotheek.",
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
            json!({"dir": dir.to_string_lossy(), "read_only": false}),
        );
        assert_eq!(s.banner(), Some(banner));
        assert_eq!(refused(&mut s), save_as);
        assert!(!target.exists(), "nothing is written");
        assert!(
            s.read_only_reason().is_some(),
            "the session stays on the read-only project"
        );
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}

/// ID-1427 (W52 F7): the damaged copy's banner stays while the project is read-only for it: after a page change it
/// still names the device and the way out (the open's finding count goes with the message), and it cannot be
/// dismissed; it follows the UI language, and it goes when the project closes.
#[test]
fn id1427_the_damaged_copy_banner_stays_while_its_state_holds() {
    let tmp = Tmp::new("id1427-damaged-banner");
    let (dir, _) = damaged_golden(&tmp);
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": false}),
    );
    done(&mut s, "page.next", json!({}));
    assert_eq!(
        s.banner(),
        Some(
            "This project is open read-only: the device -201U8-Q1 uses the library copy of its part, which is \
             damaged. Update the copy from its library first."
        )
    );
    assert!(!s.banner_dismissable());
    done(&mut s, "view.language", json!({"value": "nl"}));
    assert_eq!(
        s.banner(),
        Some(
            "Dit project is alleen-lezen geopend: het apparaat -201U8-Q1 gebruikt de bibliotheekkopie van zijn \
             artikel, die beschadigd is. Werk de kopie eerst bij vanuit de bibliotheek."
        )
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    done(&mut s, "file.close", json!({}));
    assert_eq!(s.banner(), None);
    assert!(strings::set_locale("en"));
}

/// A copy of Golden A in `tmp` whose library copy `file` (under `lib/`) is cut short: the folder.
fn golden_with_damaged(tmp: &Tmp, file: &str) -> std::path::PathBuf {
    let dir = tmp.golden_copy();
    copy_dir(std::path::Path::new(common::GOLDEN_A), &dir);
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

/// FT-1500 (ID-1458): a damaged library copy that no device names, here the form a page names, is named in the banner
/// and the Save As refusal by its kind and by the record that names it, in both languages; nothing is written.
#[test]
fn ft1500_a_damaged_copy_a_page_names_is_named_with_its_page() {
    let tmp = Tmp::new("ft1500-form");
    let dir = golden_with_damaged(&tmp, "forms/81ee0d6d-6c0e-55e4-b7fd-1896f594051f.json");
    let before = tree(&dir);
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the library copy of a form named by page 1 is damaged. Update the \
             copy from its library first. 1 integrity finding was found when the project was opened.",
            "Save as is refused: the library copy of a form named by page 1 is damaged, and a copy of the project \
             without it would not open. Update the copy from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een formulier waarnaar pagina 1 verwijst \
             is beschadigd. Werk de kopie eerst bij vanuit de bibliotheek. Bij het openen van het project is 1 \
             integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: de bibliotheekkopie van een formulier waarnaar pagina 1 verwijst is \
             beschadigd, en een projectkopie zonder deze bibliotheekkopie zou niet openen. Werk de kopie eerst bij \
             vanuit de bibliotheek.",
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
            json!({"dir": dir.to_string_lossy(), "read_only": false}),
        );
        assert_eq!(s.banner(), Some(banner));
        let message = match s.dispatch("file.save_as", &json!({"dir": target.to_string_lossy()})) {
            Outcome::Failed { message } => message,
            other => panic!("{other:?}"),
        };
        assert_eq!(message, save_as);
        assert!(!target.exists(), "nothing is written");
        assert!(s.read_only_reason().is_some());
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
    assert!(tree(&dir) == before, "nothing is written inside the project");
}

/// FT-1500 (ID-1458): a damaged library copy that no record names (a manufacturer, which only part copies' library
/// links name) leaves the project read-only with a banner that names it and the way out; Save As then makes a copy of
/// the project without it, which opens for editing, and its toast names the copy left out; the banner goes. The
/// damaged project is never written.
#[test]
fn ft1500_save_as_leaves_out_a_damaged_copy_no_record_names() {
    let tmp = Tmp::new("ft1500-left-out");
    let file = "manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    let dir = golden_with_damaged(&tmp, file);
    let before = tree(&dir);
    for (locale, banner, toast) in [
        (
            "en",
            "This project is open read-only: the library copy of a manufacturer is damaged. Update the copy from \
             its library, or use Save as for a copy of the project without it. 1 integrity finding was found when \
             the project was opened.",
            "Saved as save-as-en without the library copy of a manufacturer, which is damaged.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: de bibliotheekkopie van een fabrikant is beschadigd. Werk de \
             kopie bij vanuit de bibliotheek, of maak met Opslaan als een projectkopie zonder deze \
             bibliotheekkopie. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opgeslagen als save-as-nl zonder de bibliotheekkopie van een fabrikant, die beschadigd is.",
        ),
    ] {
        let target = tmp.0.join(format!("save-as-{locale}"));
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

/// FT-1525 (ID-1458): a damaged copy of a part an installed library holds (the compiled-in starter parts, assigned to
/// F55 and saved) is named in the banner and the Save As refusal by its part number and its library, whole in both
/// languages.
#[test]
fn ft1525_a_damaged_copy_of_an_installed_part_is_named_whole() {
    let tmp = Tmp::new("ft1525");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "select.set", json!({ "refs": ["tag:F55"] }));
    done(&mut s, "part.choose", json!({ "use": 0 }));
    done(&mut s, "part.pick", json!({ "part": "EW-GEN-MCB-2" }));
    let params = s.recorded_commands().last().expect("the assignment").params.clone();
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
    let dir = tmp.golden_copy();
    std::fs::write(
        dir.join("lib/parts").join(format!("{copy}.json")),
        "{\"$k\": \"part\",\n",
    )
    .unwrap();
    let target = tmp.0.join("save-as");
    for (locale, banner, save_as) in [
        (
            "en",
            "This project is open read-only: the device F55 uses the library copy of part EW-GEN-MCB-2 from \
             ElektraWatt starter parts, which is damaged. Update the copy from its library first. 1 integrity \
             finding was found when the project was opened.",
            "Save as is refused: the device F55 uses the library copy of part EW-GEN-MCB-2 from ElektraWatt \
             starter parts, which is damaged, and a copy of the project without it would not open. Update the copy \
             from its library first.",
        ),
        (
            "nl",
            "Dit project is alleen-lezen geopend: het apparaat F55 gebruikt de bibliotheekkopie van artikel \
             EW-GEN-MCB-2 uit ElektraWatt starter parts, die beschadigd is. Werk de kopie eerst bij vanuit de \
             bibliotheek. Bij het openen van het project is 1 integriteitsbevinding gevonden.",
            "Opslaan als is geweigerd: het apparaat F55 gebruikt de bibliotheekkopie van artikel EW-GEN-MCB-2 uit \
             ElektraWatt starter parts, die beschadigd is, en een projectkopie zonder deze bibliotheekkopie zou \
             niet openen. Werk de kopie eerst bij vanuit de bibliotheek.",
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
            json!({"dir": dir.to_string_lossy(), "read_only": false}),
        );
        assert_eq!(s.banner(), Some(banner));
        let message = match s.dispatch("file.save_as", &json!({"dir": target.to_string_lossy()})) {
            Outcome::Failed { message } => message,
            other => panic!("{other:?}"),
        };
        assert_eq!(message, save_as);
        assert!(!target.exists(), "nothing is written");
        done(&mut s, "file.close", json!({}));
    }
    assert!(strings::set_locale("en"));
}
