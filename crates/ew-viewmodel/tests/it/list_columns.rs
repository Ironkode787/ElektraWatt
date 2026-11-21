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
        "reason-no-list-page"
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
