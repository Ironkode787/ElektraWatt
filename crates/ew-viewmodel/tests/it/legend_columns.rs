// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A legend chooses its printed columns as a list does (doc 09 §3.8, FT-1772): on a copy of the sample's panel layout
//! 902, a legend made before its block had columns of its own prints the report's four columns, and List columns…,
//! with the legend selected, offers the description among the others; its Apply makes the legend print it.

use crate::common;

use common::{Tmp, done, form};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::shell::{FormTable, Mode, TICKED};
use serde_json::{Map, Value, json};

/// The rows of `table` that are ticked, by their column's name.
fn ticked(table: &FormTable) -> Vec<String> {
    table
        .rows
        .iter()
        .filter(|r| r.cells[0] == TICKED)
        .map(|r| r.key.clone())
        .collect()
}

#[test]
fn an_old_legend_takes_the_description_through_list_columns() {
    let tmp = Tmp::new("legend-columns");
    let mut s = tmp.sample_in(Mode::Headless, true, "902");
    let legend = s
        .panel_tables()
        .and_then(|p| p.blocks.first().map(|b| b.id))
        .expect("the sample's legend on 902");
    // The legend as a build before 0.3 made it: no columns of its own.
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {
            "kind": "report_block", "targets": [legend.to_string()], "path": "columns", "value": []
        }}),
    );
    // Nothing selected on a drawn page: refused in words.
    done(&mut s, "select.set", json!({ "ids": [] }));
    assert_eq!(
        common::refused(&mut s, "report_block.columns", json!({})),
        "reason-no-list-or-legend"
    );
    // The legend selected: its menu offers List columns…, whose form ticks the four columns it prints.
    done(&mut s, "select.set", json!({ "ids": [legend.to_string()] }));
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    assert!(
        menu.iter().any(|i| i.id == "report_block.columns" && i.enabled),
        "{menu:?}"
    );
    done(&mut s, "report_block.columns", json!({}));
    let f = form(&s);
    assert!(f.fields.is_empty(), "a legend prints no caption: {:?}", f.fields);
    let mut table = f.table.expect("the columns' table");
    assert_eq!(ticked(&table), ["Pos", "DT", "Qty", "Part number"]);
    let description = table
        .rows
        .iter()
        .position(|r| r.key == "Description")
        .expect("the description is offered");
    table.rows[description].cells[0] = TICKED.to_owned();
    let param: Vec<Value> = table
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
        .collect();
    done(&mut s, "report_block.columns", json!({ "columns": param }));
    // Opened again, the legend prints the description after its four columns.
    done(&mut s, "report_block.columns", json!({}));
    let again = form(&s).table.expect("the columns' table");
    assert_eq!(ticked(&again), ["Pos", "DT", "Qty", "Part number", "Description"]);
}
