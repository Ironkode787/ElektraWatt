// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ List columns… and Rows of your own… (doc 09 §3.8, FT-1874) on a copy of the sample's terminal diagram,
//! 903: the forms show what the list prints, their Apply writes it to the list's block, and opening them again shows
//! what was written; a page that is not a list's refuses them in words.

use crate::common;

use common::{Tmp, done, form};
use ew_viewmodel::shell::{FormTable, Mode, Outcome, TICKED};
use serde_json::{Map, Value, json};

/// The table parameter of `table` with its cells as given: one object per row, its cells by column name.
fn param(table: &FormTable) -> Value {
    Value::Array(
        table
            .rows
            .iter()
            .map(|r| {
                let values: Map<String, Value> = table
                    .columns
                    .iter()
                    .zip(&r.cells)
                    .filter(|(_, c)| !c.is_empty())
                    .map(|((name, _), c)| (name.clone(), json!(c)))
                    .collect();
                json!({"name": r.key, "values": values})
            })
            .collect(),
    )
}

#[test]
fn list_columns_and_rows_of_your_own_write_the_list_on_show() {
    let tmp = Tmp::new("list-columns");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    assert_eq!(
        common::refused(&mut s, "report_block.columns", json!({})),
        "reason-no-list-or-legend"
    );
    done(&mut s, "page.goto", json!({"folio": "903"}));
    done(&mut s, "report_block.columns", json!({}));
    let f = form(&s);
    let mut table = f.table.clone().expect("the columns' table");
    // The list prints its kind's columns: every row ticked, numbered in order.
    assert!(table.rows.iter().all(|r| r.cells[0] == TICKED), "{table:?}");
    let row = |t: &FormTable, key: &str| {
        t.rows
            .iter()
            .position(|r| r.key == key)
            .unwrap_or_else(|| panic!("a row {key}"))
    };
    let (terminal, side1) = (row(&table, "Terminal"), row(&table, "Side 1 (clamp a)"));
    // Print the first side, then the terminal under a heading of its own.
    for (i, r) in table.rows.iter_mut().enumerate() {
        if i != terminal && i != side1 {
            r.cells[0].clear();
        }
    }
    table.rows[side1].cells[3] = "1".to_owned();
    table.rows[terminal].cells[3] = "2".to_owned();
    table.rows[terminal].cells[1] = "Klem".to_owned();
    let apply = json!({
        "caption": "Terminal list",
        "legend": "PE: protective earth",
        "columns": param(&table),
    });
    // A width that is no whole number of 1 or more is refused in words, in the form.
    let mut wrong = table.clone();
    wrong.rows[terminal].cells[2] = "0".to_owned();
    let mut bad = apply.clone();
    bad["columns"] = param(&wrong);
    let refusal = common::failed(&mut s, "report_block.columns", bad);
    assert!(
        refusal.starts_with("The width of Terminal is a whole number"),
        "{refusal}"
    );
    done(&mut s, "report_block.columns", apply);
    assert!(s.dialog().is_none(), "{:?}", s.dialog());
    // Opened again, the form shows what the list prints now.
    done(&mut s, "report_block.columns", json!({}));
    let f = form(&s);
    let field = |name: &str| f.fields.iter().find(|x| x.name == name).map(|x| x.value.clone());
    assert_eq!(field("caption").as_deref(), Some("Terminal list"));
    assert_eq!(field("legend").as_deref(), Some("PE: protective earth"));
    let t = f.table.clone().expect("the table");
    let printed: Vec<(&str, &str)> = t
        .rows
        .iter()
        .filter(|r| r.cells[0] == TICKED)
        .map(|r| (r.key.as_str(), r.cells[1].as_str()))
        .collect();
    assert_eq!(printed, [("Side 1 (clamp a)", ""), ("Terminal", "Klem")]);
    assert_eq!(s.dispatch("ui.dismiss", &json!({})), Outcome::Done);
    // Rows of your own: the printed columns, under their headings, and empty rows to fill.
    done(&mut s, "report_block.rows", json!({}));
    let mut rows = form(&s).table.expect("the rows' table");
    let names: Vec<&str> = rows.columns.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Side 1 (clamp a)", "Klem"]);
    assert!(rows.rows.iter().all(|r| r.cells.iter().all(String::is_empty)));
    rows.rows[0].cells = vec!["-Q9:1".to_owned(), "1a".to_owned()];
    done(&mut s, "report_block.rows", json!({"rows": param(&rows)}));
    done(&mut s, "report_block.rows", json!({}));
    let again = form(&s).table.expect("the rows' table");
    assert_eq!(again.rows[0].cells, ["-Q9:1", "1a"]);
    assert!(
        again.rows[1..]
            .iter()
            .all(|r| r.cells.iter().all(String::is_empty))
    );
}

/// The text of row `key` of the window's property sheet.
fn row_text(s: &ew_viewmodel::shell::ShellModel, key: &str) -> String {
    let sheet = s.properties().sheet().expect("a sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"));
    ew_viewmodel::properties::value_text(row)
}

/// A list's title prints in the project's language unless a person typed one (doc 09 §3.8): the standard layout's
/// 903 and a new list without a title of its own print Dutch in a Dutch project; a typed title stays as typed.
#[test]
fn a_lists_title_prints_in_the_projects_language_unless_typed() {
    let tmp = Tmp::new("list-title");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "project.languages.set", "params": {"source": "nl-NL"}}),
    );
    let title = |s: &ew_viewmodel::shell::ShellModel, folio: &str| {
        s.page_rows()
            .iter()
            .find(|r| r.folio == folio)
            .map(|r| r.title.clone())
            .unwrap_or_else(|| panic!("a page {folio}"))
    };
    assert_eq!(title(&s, "903"), "Klemmenschema 1");
    done(
        &mut s,
        "report_block.new",
        json!({"kind": "terminal_diagram", "folio": "950"}),
    );
    let dutch = ew_viewmodel::strings::shipped_texts("report-kind-terminal-diagram")
        .into_iter()
        .find(|(tag, _)| *tag == "nl-NL")
        .map(|(_, t)| t)
        .expect("the Dutch name");
    assert_eq!(title(&s, "950"), dutch);
    done(
        &mut s,
        "report_block.new",
        json!({"kind": "wire_list", "folio": "960", "title": "Draden"}),
    );
    assert_eq!(title(&s, "960"), "Draden");
}

/// A printed column's row in Properties names the column by its heading in the window's language, never by its key.
#[test]
fn a_printed_columns_row_reads_its_heading_not_its_key() {
    let tmp = Tmp::new("list-column-row");
    let mut s = tmp.sample_in(Mode::Headless, true, "903");
    done(&mut s, "report_block.columns", json!({}));
    let mut table = form(&s).table.expect("the columns' table");
    for r in table.rows.iter_mut().skip(1) {
        r.cells[0].clear();
    }
    let strip = table.rows[0].key.clone();
    done(
        &mut s,
        "report_block.columns",
        json!({ "columns": param(&table) }),
    );
    assert_eq!(row_text(&s, "report_block/columns[0].key"), strip);
    done(&mut s, "view.language", json!({ "value": "nl" }));
    let column = row_text(&s, "report_block/columns[0].key");
    assert_ne!(column, "strip");
    assert_ne!(column, strip, "the Dutch heading");
    assert_eq!(row_text(&s, "report_block/columns"), column);
}

/// A list's Form row names its form in the window's language (doc 09 §3.8): the row, its choices and a name typed in
/// that language, never an ID.
#[test]
fn a_lists_form_row_reads_in_the_windows_language() {
    let tmp = Tmp::new("list-form-row");
    let mut s = tmp.sample_in(Mode::Headless, true, "903");
    // The list's block itself, as the Report blocks navigator selects it: its rows are edited there.
    let block = s
        .properties()
        .sheet()
        .expect("a sheet")
        .sections
        .iter()
        .find(|x| x.kind == "report_block")
        .expect("the list's block")
        .targets[0];
    done(&mut s, "select.set", json!({ "ids": [block.to_string()] }));
    let frame = |lang: &str| {
        ew_viewmodel::strings::shipped_texts("library-form-frame")
            .into_iter()
            .find(|(tag, _)| *tag == lang)
            .map(|(_, t)| t)
            .expect("the word")
    };
    let english = row_text(&s, "report_block/form");
    assert!(english.starts_with(frame("en-GB")), "{english}");
    done(&mut s, "view.language", json!({ "value": "nl" }));
    let dutch = row_text(&s, "report_block/form");
    assert!(dutch.starts_with(frame("nl-NL")), "{dutch}");
    done(&mut s, "props.edit", json!({ "row": "report_block/form" }));
    let items = s.properties().choices().expect("the forms").items.clone();
    assert!(items.iter().any(|(t, _)| *t == dutch), "{items:?}");
    done(&mut s, "props.cancel", json!({}));
    let (other, value) = items
        .iter()
        .find(|(t, _)| *t != dutch && t.starts_with(frame("nl-NL")))
        .cloned()
        .expect("another frame");
    done(
        &mut s,
        "props.set",
        json!({ "row": "report_block/form", "value": other }),
    );
    assert_eq!(row_text(&s, "report_block/form"), other);
    let sheet = s.properties().sheet().expect("a sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .find(|r| r.key == "report_block/form")
        .expect("the Form row");
    assert!(
        matches!(&row.value, ew_viewmodel::properties::RowValue::One { json, .. } if *json == value),
        "{:?}",
        row.value
    );
}
