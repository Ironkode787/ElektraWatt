// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The project's Frames row names its frames (FT-1916): the sample's default frame and each page type's own, by the
//! names the Form row's choices give them, never by a form's ID.

use crate::common::{Tmp, done};
use ew_viewmodel::properties::value_text;
use serde_json::json;

#[test]
fn the_frames_row_names_the_frames() {
    let tmp = Tmp::new("project-frames-words");
    let mut s = tmp.open_sample(false);
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.show_advanced", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let sheet = s.properties().sheet().expect("the project's sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .find(|r| r.key == "project/settings.frames")
        .expect("the Frames row");
    let ids: Vec<String> = {
        let json = match &row.value {
            ew_viewmodel::properties::RowValue::One { json, .. } => json.clone(),
            other => panic!("{other:?}"),
        };
        std::iter::once(json["default"].clone())
            .chain(
                json["per_page_type"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| e["form"].clone()),
            )
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect()
    };
    let text = value_text(row);
    assert!(text.starts_with("Default: Frame "), "{text}");
    assert!(text.contains("; Table of contents: "), "{text}");
    for id in ids {
        assert!(!text.contains(&id), "{text} names {id}");
    }
}
