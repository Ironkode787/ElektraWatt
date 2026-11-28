// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Edit frame on a copy of the sample (doc 09 §3.9, the journey's steps 2 to 4): a cell's text and what it shows, a
//! cell split one above the other, a field of the project's own and one of each page made from the cells; the first
//! change makes the starter title block the project's own; Title block fields… then lists the page's field first and
//! writes it in one step.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

/// The index of the title block's cell whose printed text starts with `start`.
fn cell(s: &ShellModel, start: &str) -> usize {
    let facts = s.frame_facts().expect("the frame of page 101");
    facts
        .cells
        .iter()
        .find(|c| c.printed.starts_with(start))
        .unwrap_or_else(|| panic!("a cell printing {start}: {:?}", facts.cells))
        .index
}

/// The text page 101 prints in the cell `index`.
fn printed(s: &ShellModel, index: usize) -> String {
    let facts = s.frame_facts().expect("the frame");
    facts
        .cells
        .iter()
        .find(|c| c.index == index)
        .expect("the cell")
        .printed
        .clone()
}

#[test]
fn the_journey_changes_the_title_block_on_the_page_and_its_fields_are_typed_in_title_block_fields() {
    let tmp = Tmp::new("frame-edit-journey");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "frame.edit", json!({}));
    assert_eq!(s.tool().name(), "frame");
    assert!(
        !s.frame_facts().expect("the frame").block_local,
        "the starter's block"
    );

    // J2: "End user: …" becomes "Customer:", showing the customer's name.
    let end_user = cell(&s, "End user");
    done(&mut s, "frame.choose", json!({"cells": [end_user]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/cell.text", "value": "Customer:"}),
    );
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/cell.shows", "value": "party.customer.name"}),
    );
    let facts = s.frame_facts().expect("the frame");
    assert!(
        facts.block_local,
        "the first change makes the block the project's own"
    );
    assert!(
        printed(&s, end_user).starts_with("Customer:"),
        "{}",
        printed(&s, end_user)
    );

    // J3: the empty notice cell split one above the other; the upper half prints a field of the whole project.
    let notice = facts
        .cells
        .iter()
        .find(|c| c.value.as_deref() == Some("{project.notice}"))
        .expect("the notice cell")
        .index;
    done(&mut s, "frame.choose", json!({"cells": [notice]}));
    done(&mut s, "frame.split", json!({"dir": "one_above"}));
    done(&mut s, "frame.choose", json!({"cells": [notice]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/cell.text", "value": "Order:"}),
    );
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/cell.shows", "value": "new_field"}),
    );
    done(
        &mut s,
        "field.new",
        json!({"name": "Order number", "for": "whole_project", "value": "0001"}),
    );
    assert_eq!(printed(&s, notice), "Order: 0001");

    // J4: the lower half prints a field of each page, its value this page's.
    done(&mut s, "frame.choose", json!({"cells": [notice + 1]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/cell.shows", "value": "new_field"}),
    );
    done(
        &mut s,
        "field.new",
        json!({"name": "Customer sheet", "for": "each_page", "value": "C-101"}),
    );
    assert_eq!(printed(&s, notice + 1), "Customer sheet: C-101");
    done(&mut s, "frame.edit", json!({}));
    assert_eq!(s.tool().name(), "select", "Done ends the tool");

    // Title block fields… lists the page's field first and writes it in one step.
    done(&mut s, "page.title_block", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the form: {:?}", s.dialog())
    };
    assert_eq!(form.fields[0].name, "page.prop.customer_sheet");
    assert_eq!(form.fields[0].label.as_deref(), Some("Customer sheet"));
    assert_eq!(form.fields[0].value, "C-101");
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let before = s.revision();
    done(
        &mut s,
        "page.title_block",
        json!({"page.prop.customer_sheet": "C-102"}),
    );
    assert_eq!(s.revision(), before.map(|r| r + 1), "one step");
    assert_eq!(printed(&s, notice + 1), "Customer sheet: C-102");
}

/// The journey's step 5: a click on the rulers' row chooses them; How many, First and the rows' count typed in
/// Properties open the Columns and rows card, whose preview lists the cross-references that move; Apply writes them in
/// one step, and page 101 then counts 10 columns from 1 and 6 rows.
#[test]
fn columns_and_rows_are_set_in_properties_and_applied_from_their_card() {
    let tmp = Tmp::new("frame-edit-rulers");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "frame.edit", json!({}));
    done(&mut s, "frame.choose", json!({"rulers": true}));
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/columns.count", "value": "10"}),
    );
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/columns.first", "value": "1"}),
    );
    done(
        &mut s,
        "props.set",
        json!({"row": "frame/rows.count", "value": "6"}),
    );
    assert!(s.properties().card().is_some(), "the card waits for Apply");
    let refused = common::failed(
        &mut s,
        "props.set",
        json!({"row": "frame/rows.first", "value": "7"}),
    );
    assert_eq!(refused, "Type a letter, such as A.");
    done(&mut s, "preview.apply", json!({}));
    let facts = s.frame_facts().expect("the frame");
    let columns = facts.columns.clone().expect("columns");
    let rows = facts.rows.clone().expect("rows");
    assert_eq!(
        (
            columns.count,
            columns.first.as_str(),
            rows.count,
            rows.first.as_str()
        ),
        (10, "1", 6, "A")
    );
}

/// The groups of the frame's Properties with the labels of their rows, in display order.
fn frame_groups(s: &ShellModel) -> Vec<(String, Vec<String>)> {
    let sheet = s.properties().sheet().expect("the frame's sheet");
    ew_viewmodel::properties::groups(&sheet.sections[0], false)
        .into_iter()
        .map(|(g, rows)| {
            let labels = rows.iter().map(|r| r.label.clone().unwrap_or_default()).collect();
            (g.title().to_owned(), labels)
        })
        .collect()
}

/// The rulers' rows stand under Columns, along the top, Rows, along the left and Both, each labelled with its short
/// word; a cell's rows under Cell, the frame's under Frame: no row of the frame waits under Other.
#[test]
fn the_frames_rows_stand_in_the_sections_the_design_draws() {
    let tmp = Tmp::new("frame-edit-sections");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "frame.edit", json!({}));
    let titles = |g: &[(String, Vec<String>)]| g.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&frame_groups(&s)), ["Frame"]);
    done(&mut s, "frame.choose", json!({"rulers": true}));
    let groups = frame_groups(&s);
    assert_eq!(
        titles(&groups),
        ["Columns, along the top", "Rows, along the left", "Both"]
    );
    assert_eq!(groups[0].1, ["How many", "Counted with", "First"]);
    assert_eq!(groups[1].1, ["How many", "Counted with", "First", "Skip I and O"]);
    let end_user = cell(&s, "End user");
    done(&mut s, "frame.choose", json!({"cells": [end_user]}));
    assert_eq!(titles(&frame_groups(&s)), ["Cell"]);
}

/// A double click on a chosen cell opens its text in the free-text editor: Enter writes what was typed, in one step;
/// Esc leaves the cell as it was.
#[test]
fn a_double_click_types_a_cells_text_in_the_free_text_editor() {
    let tmp = Tmp::new("frame-edit-typing");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "frame.edit", json!({}));
    let end_user = cell(&s, "End user");
    done(&mut s, "frame.choose", json!({"cells": [end_user]}));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    let popup = s.text_popup().expect("the free-text editor");
    assert!(popup.text.starts_with("End user"), "{}", popup.text);
    let _ = s.dispatch("ui.dismiss", &json!({}));
    assert_eq!(s.tool().state(), "cell", "Esc keeps the cell and the tool");
    assert!(printed(&s, end_user).starts_with("End user"));

    done(&mut s, "tool.finish", json!({}));
    done(&mut s, "text.input", json!({"text": "Client:"}));
    let before = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.revision(), before.map(|r| r + 1), "one step");
    assert!(
        printed(&s, end_user).starts_with("Client:"),
        "{}",
        printed(&s, end_user)
    );
    assert_eq!(s.tool().state(), "cell");
}

/// The text of the frame's row `key` in the open sheet.
fn frame_row(s: &ShellModel, key: &str) -> String {
    let sheet = s.properties().sheet().expect("the frame's sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|sec| sec.rows.iter())
        .find(|r| r.key == format!("frame/{key}"))
        .unwrap_or_else(|| panic!("no row {key}"));
    ew_viewmodel::properties::value_text(row)
}

/// The frame's lengths read as every Properties row writes one: the shortest millimetres with the project's
/// decimal separator, never "297.0" with a point in a project that writes a comma.
#[test]
fn the_frames_lengths_read_as_every_length_of_properties() {
    let tmp = Tmp::new("frame-edit-lengths");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "frame.edit", json!({}));
    let paper = frame_row(&s, "frame.paper");
    assert!(paper.ends_with(" mm") && !paper.contains(".0 "), "{paper}");
    done(&mut s, "frame.choose", json!({"rulers": true}));
    let height = frame_row(&s, "both.block_height");
    assert!(
        !height.ends_with(".0") && height.parse::<f64>().is_ok(),
        "{height}"
    );
    s.shutdown();
    let file = tmp.sample_copy().join("project.jsonl");
    let jsonl = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        jsonl.replace(
            "\"decimal_separator\":\"point\"",
            "\"decimal_separator\":\"comma\"",
        ),
    )
    .unwrap();
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "frame.edit", json!({}));
    done(&mut s, "frame.choose", json!({"rulers": true}));
    let height = frame_row(&s, "both.block_height");
    assert!(!height.contains('.'), "{height}");
    s.shutdown();
}
