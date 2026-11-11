// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 45's Properties polish through the shell on temporary copies of Golden A: `props.follow` on a
//! placement's Device row and 902's Placement rows in words.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::ew_api::output::{PanelTables, RowValue};
use ew_viewmodel::properties::{row_label, value_text, visible_rows};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

fn panel(shell: &ShellModel) -> std::sync::Arc<PanelTables> {
    shell.panel_tables().expect("a panel page with a surface view")
}

/// The (label, value) pairs of the rows on show, Advanced on or off.
fn shown(s: &ShellModel, advanced: bool) -> Vec<(String, String)> {
    let sheet = s.properties().sheet().expect("a Properties sheet");
    visible_rows(sheet, &s.properties_collapsed(), advanced, "")
        .into_iter()
        .map(|v| (row_label(v.section, v.row), value_text(v.row)))
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

/// "Properties' Device row is a link in the accent colour that a click or F (`props.follow`, in the
/// properties context of every keymap, through the action registry; typed, not followed, while an editor field has
/// the focus) follows to the device on its main page". `props.follow` on -102F1's placement shows -102F1's main page
/// (102) with its drawings there selected, so Properties shows the device; on a row that links nothing it is refused.
#[test]
fn props_follow_goes_from_a_placement_to_its_device_on_its_main_page() {
    let tmp = Tmp::new("props-follow");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    done(&mut s, "props.show", json!({"on": true}));
    let f1 = panel(&s)
        .placements
        .iter()
        .find(|p| p.tag == "-102F1")
        .unwrap()
        .id;
    done(&mut s, "select.set", json!({"ids": [f1.to_string()]}));
    let sheet = s.properties().sheet().expect("a sheet");
    let device = sheet.sections[0]
        .rows
        .iter()
        .find(|r| r.key == "placement/derived.device")
        .expect("the Device row");
    assert!(matches!(&device.value, RowValue::One { text, .. } if text == "-102F1"));
    // A row that links nothing: refused in words.
    assert_eq!(
        refused(
            &mut s,
            "props.follow",
            json!({"row": "placement/derived.position"})
        ),
        "reason-row-no-link"
    );
    // A sentence with its next step.
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::key!("reason-row-no-link")),
        "The property row links to nothing. Choose a row that names a device to go to it."
    );
    assert_eq!(s.current_folio(), Some("902"));
    // The focused row, moved to the Device row, followed.
    done(&mut s, "props.row", json!({"to": "placement/derived.device"}));
    done(&mut s, "props.follow", json!({}));
    assert_eq!(s.current_folio(), Some("102"));
    assert!(!s.selection().items.is_empty());
    let sheet = s.properties().sheet().expect("the device's sheet");
    assert_eq!(sheet.header.title, "-102F1");
    s.shutdown();
}

/// The keymaps bind `props.follow` to F in the Properties context, as `nav.follow` is F on the canvas.
#[test]
fn f_follows_in_the_properties_context() {
    let tmp = Tmp::new("props-follow-key");
    let s = tmp.shell_in(Mode::Headless, false, "902");
    let keys: Vec<(String, String)> = s
        .bindings()
        .into_iter()
        .filter(|b| b.action == "props.follow")
        .map(|b| (b.chord.text(), format!("{:?}", b.context)))
        .collect();
    assert_eq!(keys, pairs(&[("F", "Properties")]));
}

/// "a placement's rows show labels and values in the user's words, and the members with no meaning
/// to the user move to Advanced (doc 16a §6.8)". 902's Placement rows: Scale "1:10"; the form's ID and the connection
/// grid's fraction only under Advanced, the grid itself as the status bar writes it. The scale is edited in
/// its words.
#[test]
fn the_placement_rows_of_902_read_in_words_and_the_scale_edits_as_it_shows() {
    let tmp = Tmp::new("ft1019");
    let mut s = tmp.shell_in(Mode::Headless, true, "902");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "select.set", json!({"ids": []}));
    let rows = shown(&s, false);
    let placement: Vec<(String, String)> = rows
        .iter()
        .filter(|(l, _)| {
            [
                "Scale",
                "Connection grid",
                "Grid step divisor",
                "Grid step multiplier",
                "Form",
            ]
            .contains(&l.as_str())
        })
        .cloned()
        .collect();
    // The Connection grid row is a choice outside Advanced; its fraction stays under it.
    assert_eq!(
        placement,
        pairs(&[("Connection grid", "QET 10 px"), ("Scale", "1:10")])
    );
    let advanced: Vec<(String, String)> = shown(&s, true)
        .into_iter()
        .filter(|(l, _)| {
            [
                "Scale",
                "Connection grid",
                "Grid step divisor",
                "Grid step multiplier",
                "Form",
            ]
            .contains(&l.as_str())
        })
        .collect();
    assert_eq!(
        advanced,
        pairs(&[
            ("Form", "c80af135-34f8-574c-8f52-756bd24c7bd0"),
            ("Connection grid", "QET 10 px"),
            ("Grid step divisor", "3"),
            ("Grid step multiplier", "7.9375 mm"),
            ("Scale", "1:10"),
        ])
    );
    done(&mut s, "props.edit", json!({"row": "page/scale"}));
    assert_eq!(s.properties().edit().map(|e| e.buffer.as_str()), Some("1:10"));
    done(&mut s, "props.input", json!({"text": "1:5"}));
    assert_eq!(s.dispatch("props.commit", &json!({})), Outcome::Done);
    let sheet = s.properties().sheet().expect("a sheet");
    let scale = sheet
        .sections
        .iter()
        .flat_map(|sec| &sec.rows)
        .find(|r| r.key == "page/scale")
        .expect("the Scale row");
    assert_eq!(value_text(scale), "1:5");
    s.shutdown();
}

/// The (label, value) pairs of the rows on show under the Properties filter `filter`.
fn filtered(s: &ShellModel, filter: &str) -> Vec<(String, String)> {
    let sheet = s.properties().sheet().expect("a Properties sheet");
    visible_rows(sheet, &s.properties_collapsed(), true, filter)
        .into_iter()
        .map(|v| (row_label(v.section, v.row), value_text(v.row)))
        .collect()
}

/// FT-1357 and FT-1358 (ruling 8): the project's sheet is headed "Project", its yes and no, list
/// counts and structured values read in words in either language, never as JSON; a filter shows its rows with their
/// parents, so the four parties' "Personal data" rows never stand alone (doc 16a §6.8).
#[test]
fn the_project_sheet_reads_in_words_and_a_filter_keeps_the_parents() {
    let tmp = Tmp::new("ft1357");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let values = |s: &ShellModel| {
        let sheet = s.properties().sheet().expect("a sheet");
        let rows = shown(s, true);
        let value = |label: &str| {
            rows.iter()
                .find(|(l, _)| l == label)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        (
            sheet.header.title.clone(),
            [
                value("Parties"),
                value("Neutral"),
                value("Colour profile"),
                value("Numbering"),
            ],
        )
    };
    assert_eq!(
        values(&s),
        (
            "Project".to_owned(),
            ["4 items", "yes", "DBU/WH 24 V", "page-based"].map(str::to_owned)
        )
    );
    let party = |n: u8| {
        [
            (format!("Parties {n}"), String::new()),
            ("Personal data".to_owned(), "yes".to_owned()),
        ]
    };
    let mut rows = vec![("Parties".to_owned(), "4 items".to_owned())];
    rows.extend((1..=4).flat_map(party));
    assert_eq!(filtered(&s, "personal"), rows);
    done(&mut s, "view.language", json!({"value": "nl"}));
    let sheet = s.properties().sheet().expect("a sheet");
    assert_eq!(sheet.header.title, "Project");
    let texts: Vec<String> = [
        "project/parties",
        "project/supply.neutral",
        "project/settings.numbering",
    ]
    .iter()
    .map(|k| {
        let row = sheet.sections[0]
            .rows
            .iter()
            .find(|r| r.key == *k)
            .expect("the row");
        value_text(row)
    })
    .collect();
    assert_eq!(texts, ["4 elementen", "ja", "page-based"]);
    s.shutdown();
}
