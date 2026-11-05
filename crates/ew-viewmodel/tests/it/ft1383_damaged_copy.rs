// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1383 (ID-1395): a copy of Golden A whose one library copy that a device names is damaged opens read-only in
//! the window, as a damaged copy no device names does, instead of being refused whole: every page shows and the
//! checks run.

use crate::common;

use common::{Tmp, copy_dir, done};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

#[test]
fn ft1383_a_damaged_copy_a_device_names_opens_read_only_and_every_page_shows() {
    let tmp = Tmp::new("ft1383");
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
