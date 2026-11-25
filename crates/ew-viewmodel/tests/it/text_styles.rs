// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text styles in the window: a person changes the heights, typefaces and bold
//! of the project's texts on one page, by names he knows; the warning about a text too small to read takes him there;
//! a new project starts readable.

use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

use crate::common::{Tmp, done, new_project};

/// The open findings of `rule` after Run checks: (row key, label).
fn findings(shell: &mut ShellModel, rule: &str) -> Vec<(String, String)> {
    done(shell, "messages.show", json!({"on": true}));
    done(shell, "messages.check", json!({}));
    let t = shell
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .expect("the Messages table");
    t.nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule)
        .map(|n| (n.key.clone(), n.label.clone()))
        .collect()
}

/// The page's row of the style named `name`, as `ui.describe` lists it.
fn row(shell: &ShellModel, name: &str) -> String {
    let text = shell.describe();
    text.lines()
        .find(|l| l.starts_with(&format!("  style \"{name}\"")))
        .unwrap_or_else(|| panic!("no style {name}:\n{text}"))
        .to_owned()
}

/// The owner met "text style cp.small prints with a cap height of 0.94 mm" with nowhere to go: the warning names the
/// style as the page does and says what to do; opening it opens Text styles at that style, whose row says the same;
/// a height typed there is one undo step and silences the warning; Standard sizes mends every style at once.
#[test]
fn a_text_too_small_to_read_opens_text_styles_where_it_is_mended() {
    let tmp = Tmp::new("text-styles-finding");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    // The sample is readable: its device tag is made too small first.
    done(&mut shell, "text_styles.open", json!({}));
    done(
        &mut shell,
        "text_styles.set",
        json!({"style": "tag.main", "height": "0.94"}),
    );
    done(&mut shell, "ui.dismiss", json!({}));
    let found = findings(&mut shell, "EW-CORE-061");
    let (key, label) = found
        .iter()
        .find(|(_, l)| l.starts_with("Device tag"))
        .cloned()
        .unwrap_or_else(|| panic!("{found:?}"));
    assert_eq!(
        label,
        "Device tag prints 0.94 mm high, too small to read on paper (1.20 mm at least). Make it larger in Text styles."
    );
    done(
        &mut shell,
        "navigator.open",
        json!({"kind": "messages", "key": key}),
    );
    let Some(Dialog::TextStyles(d)) = shell.dialog() else {
        panic!("Text styles opens: {:?}", shell.dialog());
    };
    assert_eq!(d.focus.as_deref(), Some("tag.main"));
    let marker = row(&shell, "Device tag");
    assert!(marker.contains("(opened at)"), "{marker}");
    assert!(
        marker.contains("0.94 mm") && marker.contains("too small to read on paper"),
        "{marker}"
    );

    done(
        &mut shell,
        "text_styles.set",
        json!({"style": "tag.main", "height": "2,5"}),
    );
    let marker = row(&shell, "Device tag");
    assert!(
        marker.contains(": 2.5 mm, Standard") && !marker.contains("too small"),
        "{marker}"
    );
    done(&mut shell, "edit.undo", json!({}));
    assert!(row(&shell, "Device tag").contains("0.94 mm"), "one undo step");

    done(&mut shell, "text_styles.standard", json!({}));
    assert!(row(&shell, "Device tag").contains("2.46 mm"));
    assert!(row(&shell, "Wire marker").contains("1.32 mm"));

    assert!(row(&shell, "Pin designation").contains("1.32 mm"));
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(
        findings(&mut shell, "EW-CORE-061"),
        Vec::<(String, String)>::new()
    );
}

/// A typeface and bold change the drawing's texts: the device tags of the open page print in the narrow face, bold;
/// Reset brings the style back to what a new project has.
#[test]
fn a_typeface_and_bold_reach_the_open_page_and_reset_undoes_them() {
    let tmp = Tmp::new("text-styles-face");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let before = format!("{:?}", shell.page_content("101").unwrap().pages[0].content);
    done(&mut shell, "text_styles.open", json!({}));
    done(
        &mut shell,
        "text_styles.set",
        json!({"style": "tag.main", "face": "narrow", "bold": true}),
    );
    assert!(row(&shell, "Device tag").contains(": 2.46 mm, Narrow, bold"));
    let after = format!("{:?}", shell.page_content("101").unwrap().pages[0].content);
    assert_ne!(before, after, "the open page shows the change");
    done(&mut shell, "text_styles.reset", json!({"style": "tag.main"}));
    assert!(row(&shell, "Device tag").contains(": 2.46 mm, Standard"));
    let reset = format!("{:?}", shell.page_content("101").unwrap().pages[0].content);
    assert_eq!(before, reset);
}

/// The technical-drawing face has no bold: a bold style in it shows and prints regular, and is bold again in the
/// standard face; the open page shows the face at once.
#[test]
fn the_technical_drawing_face_prints_a_bold_style_regular() {
    let tmp = Tmp::new("text-styles-technical");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let before = format!("{:?}", shell.page_content("101").unwrap().pages[0].content);
    done(&mut shell, "text_styles.open", json!({}));
    done(
        &mut shell,
        "text_styles.set",
        json!({"style": "tag.main", "face": "technical_drawing", "bold": true}),
    );
    let tag = row(&shell, "Device tag");
    assert!(tag.ends_with(": 2.46 mm, Technical drawing"), "{tag}");
    assert_ne!(
        before,
        format!("{:?}", shell.page_content("101").unwrap().pages[0].content)
    );
    done(
        &mut shell,
        "text_styles.set",
        json!({"style": "tag.main", "face": "standard"}),
    );
    assert!(row(&shell, "Device tag").ends_with(": 2.46 mm, Standard, bold"));
}

/// A height typed wrong is refused in words at the row, and nothing changes.
#[test]
fn a_height_that_is_no_height_is_refused_in_words() {
    let tmp = Tmp::new("text-styles-typed");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut shell, "text_styles.open", json!({}));
    let before = row(&shell, "Wire marker");
    for typed in ["abc", "0", "99"] {
        let out = shell.dispatch("text_styles.set", &json!({"style": "marker", "height": typed}));
        let Outcome::Failed { message } = out else {
            panic!("{typed}: {out:?}");
        };
        assert!(
            message.contains("is not a height") || message.contains("A height lies between"),
            "{message}"
        );
    }
    assert_eq!(row(&shell, "Wire marker"), before);
}

/// Every new project starts readable: no style under the minimum, and the terminal's number smaller than the device
/// tag; nothing for EW-CORE-061 to report.
#[test]
fn a_new_project_starts_readable_and_its_terminal_numbers_are_smaller_than_tags() {
    let tmp = Tmp::new("text-styles-new");
    let shell = new_project(&tmp);
    let rows = shell.text_style_rows();
    assert!(rows.iter().all(|r| r.small.is_none()), "{rows:#?}");
    let em = |key: &str| rows.iter().find(|r| r.key == key).unwrap().em;
    assert!(em("terminal") < em("tag.main"));
    assert_eq!(rows.iter().find(|r| r.key == "terminal").unwrap().height, "1.8");
}
