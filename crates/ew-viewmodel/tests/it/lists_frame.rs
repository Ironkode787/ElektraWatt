// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame of the lists is set where a person finds it: a generated page's read-only Form row says
//! why under it and offers the way to Frame of the lists in the project's Properties, which gives every list the
//! frame chosen in one step. On the sample, its parts list 901.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

const ROW: &str = "project/lists.form";
const FORM: &str = "report_block/form";

/// The text of row `key` of the sheet on show.
fn text_of(s: &ShellModel, key: &str) -> Option<String> {
    let sheet = s.properties().sheet()?;
    match &find_row(sheet, key)?.1.value {
        RowValue::One { text, .. } => Some(text.clone()),
        RowValue::Varies { texts, .. } => Some(texts.join(" | ")),
        RowValue::Absent => None,
    }
}

#[test]
fn a_lists_form_row_points_to_frame_of_the_lists_which_sets_every_list() {
    let tmp = Tmp::new("lists-frame");
    let mut s = tmp.sample_in(Mode::Headless, true, "901");
    done(&mut s, "props.show", json!({"on": true}));
    let before = text_of(&s, FORM).expect("the list's Form row");

    // The generated page's Form row refuses with its reason under it, and offers Frame of the lists.
    assert!(matches!(
        s.dispatch("props.edit", &json!({"row": FORM})),
        Outcome::Failed { .. }
    ));
    assert_eq!(s.properties().error().map(|(row, _)| row), Some(FORM));
    let offer = s.row_offer(FORM).expect("the way to where the frame is set");
    assert!(offer.label.contains("every list"), "{}", offer.label);
    for (action, params) in &offer.steps {
        done(&mut s, action, params.clone());
    }
    assert_eq!(
        s.properties().moved_row(),
        Some(ROW),
        "the cursor stands on Frame of the lists"
    );

    // Another frame chosen there goes to every list.
    done(&mut s, "props.edit", json!({"row": ROW}));
    let other = s
        .properties()
        .choices()
        .expect("the project's forms")
        .items
        .iter()
        .map(|(name, _)| name.clone())
        .find(|name| *name != before)
        .expect("a second form");
    done(&mut s, "props.set", json!({"row": ROW, "value": other}));
    assert!(toast(&s).contains(&other), "{}", toast(&s));
    assert_eq!(text_of(&s, ROW).as_deref(), Some(other.as_str()));
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(
        text_of(&s, FORM).as_deref(),
        Some(other.as_str()),
        "the list prints on it"
    );
}

/// The generated table of contents is a list: Frame of the lists frames it too, and the toast says so beside the
/// frames of the drawn pages, which stay.
/// The sample's contents on page 2.
#[test]
fn frame_of_the_lists_frames_the_table_of_contents_too() {
    let tmp = Tmp::new("lists-frame-toc");
    let mut s = tmp.sample_in(Mode::Headless, true, "2");
    done(&mut s, "props.show", json!({"on": true}));
    let before = text_of(&s, FORM).expect("the contents' Form row");
    done(&mut s, "props.project", json!({}));
    done(&mut s, "props.edit", json!({"row": ROW}));
    let other = s
        .properties()
        .choices()
        .expect("the project's forms")
        .items
        .iter()
        .map(|(name, _)| name.clone())
        .find(|name| *name != before)
        .expect("a second form");
    done(&mut s, "props.set", json!({"row": ROW, "value": other}));
    assert_eq!(
        toast(&s),
        format!(
            "Every list of the project, the table of contents among them, now prints on {other}; the frames of the \
             drawn pages and the section dividers stay as they were."
        )
    );
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(text_of(&s, FORM), Some(other), "the contents print on it");
}

/// A name that is no form of the project is refused in words, as on a list's own Form row.
#[test]
fn frame_of_the_lists_refuses_a_name_it_does_not_hold() {
    let tmp = Tmp::new("lists-frame-refuse");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let before = text_of(&s, ROW).expect("the row");
    let out = s.dispatch("props.set", &json!({"row": ROW, "value": "no such frame"}));
    assert!(matches!(out, Outcome::Failed { .. }), "{out:?}");
    assert_eq!(text_of(&s, ROW), Some(before));
}
