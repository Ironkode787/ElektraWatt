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
