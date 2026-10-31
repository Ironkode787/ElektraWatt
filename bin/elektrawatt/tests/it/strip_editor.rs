// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 SE6, G-PP's H4 rows through the built binary. On a copy of Golden A (the three strips 1X1, -X1031 and
//! -X1032 with their terminals, parts and the TBD part copies of the task's prelude; the G-PP start of
//! `tests/gpp_demo.rs` has no placed strip yet, so its tags print `-X??` until the folios are drawn), a headless
//! script clears the strips' part uses (Golden A's stand-in for the end brackets and bridges), removes Golden A's
//! BR1, BR5 and BR6 with `strip.unbridge`, adds the four end brackets with
//! `strip.add_accessory` (no `part`: the one end-bracket copy), draws BR1, BR5 and BR6 again with `strip.bridge` on
//! terminals selected by `select.set` with `tag:` references, and gives the bridges the part "TBD" by one cell edit and
//! one fill-down over the bridged rows of the strip editor; the exported parts list's End bracket and Plug-in bridge
//! rows equal rows 13 and 14 of the task's `oracle/bom.csv` (every column but `Pos`, which numbers a different list).

use crate::support;

use std::path::Path;

use ew_ui::ew_viewmodel::ew_api::{BomMode, Export, Session};
use serde_json::json;

use support::{GOLDEN_A, Tmp, all_done, copy_dir, path, run, steps};

/// The path of the `elektrawatt` binary under test (nextest sets it at run time; see `tests/headless.rs`).
// Allowed: the program boundary of a test — nextest's binary path is read once, never persisted or printed.
#[allow(clippy::disallowed_methods)]
fn bin_exe() -> std::path::PathBuf {
    std::env::var_os("NEXTEST_BIN_EXE_elektrawatt")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_BIN_EXE_elektrawatt"))) // bin-exe-fallback
}

const ORACLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/blind/tasks/gpp-101-103/oracle/bom.csv"
);

/// A CSV text as rows of fields (RFC 4180 quoting).
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' if field.is_empty() => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\r' => {}
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// The rows of a parts list whose Description is `description`, as (Qty, Unit, Manufacturer, Part number,
/// Description) read by the header's names.
fn rows_of(csv: &[Vec<String>], description: &str) -> Vec<Vec<String>> {
    let header = &csv[0];
    let col = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("no {name} column in {header:?}"))
    };
    let cols = ["Qty", "Unit", "Manufacturer", "Part number", "Description"].map(col);
    csv[1..]
        .iter()
        .filter(|r| r.get(cols[4]).is_some_and(|d| d == description))
        .map(|r| cols.iter().map(|c| r[*c].clone()).collect())
        .collect()
}

#[test]
fn se6_end_brackets_and_bridges_give_the_oracle_rows_13_and_14() {
    let exe = bin_exe();
    let tmp = Tmp::new();
    let job = tmp.path("g.ew");
    copy_dir(Path::new(GOLDEN_A), &job);
    let x1031: Vec<String> = (1..=4).map(|n| format!("tag:-X1031:{n}")).collect();
    let br5: Vec<String> = (11..=20).map(|n| format!("tag:-X1031:{n}")).collect();
    let br6: Vec<String> = (1..=5).map(|n| format!("tag:-X1032:{n}")).collect();
    let bridged: Vec<String> = (1..=4)
        .chain(11..=20)
        .map(|n| format!("-X1031:{n}"))
        .chain((1..=5).map(|n| format!("-X1032:{n}")))
        .collect();
    let script = steps(&[
        ("file.open", json!({ "dir": path(&job) })),
        // Golden A lists its end brackets and bridges as part uses of the strip devices (the as-built stand-in for
        // the H4 rows); they are cleared, so that the parts list's rows come from the accessories and the bridges.
        ("select.set", json!({"refs": ["tag:1X1"]})),
        ("props.set", json!({"row": "device/parts", "value": ""})),
        ("select.set", json!({"refs": ["tag:-X1031"]})),
        ("props.set", json!({"row": "device/parts", "value": ""})),
        ("select.set", json!({"refs": ["tag:-X1032"]})),
        ("props.set", json!({"row": "device/parts", "value": ""})),
        // Golden A's BR1, BR5 and BR6 removed whole (D11), so that the script draws them.
        ("strip.edit", json!({"strip": "-X1031"})),
        ("select.set", json!({ "refs": x1031 })),
        ("strip.unbridge", json!({})),
        ("select.set", json!({ "refs": br5 })),
        ("strip.unbridge", json!({})),
        ("select.set", json!({ "refs": br6 })),
        ("strip.unbridge", json!({})),
        // The end brackets: 1X1 at its start (the strip row of the tree) and at its end, -X1031 and -X1032 after
        // their last terminal.
        ("strip.edit", json!({"strip": "1X1"})),
        ("navigator.table", json!({"kind": "strips", "on": false})),
        ("navigator.filter", json!({"kind": "strips", "text": ""})),
        ("navigator.select", json!({"kind": "strips", "keys": ["1X1"]})),
        ("strip.add_accessory", json!({})),
        ("strip.edit", json!({"strip": "1X1"})),
        ("table.cell", json!({"kind": "strips", "step": 999})),
        ("strip.add_accessory", json!({})),
        ("strip.edit", json!({"strip": "-X1031"})),
        ("table.cell", json!({"kind": "strips", "step": 999})),
        ("strip.add_accessory", json!({"kind": "end_clamp"})),
        ("strip.edit", json!({"strip": "-X1032"})),
        ("table.cell", json!({"kind": "strips", "step": 999})),
        ("strip.add_accessory", json!({})),
        // BR1, BR5 and BR6 by tags.
        ("select.set", json!({ "refs": x1031 })),
        ("strip.bridge", json!({})),
        ("select.set", json!({ "refs": br5 })),
        ("strip.bridge", json!({})),
        ("select.set", json!({ "refs": br6 })),
        ("strip.bridge", json!({})),
        // The Bridge part: one cell, then one fill-down over the bridged rows of both strips.
        ("navigator.filter", json!({"kind": "strips", "text": "x103"})),
        (
            "table.cell",
            json!({"kind": "strips", "row": "-X1031:1", "column": "bridge_part"}),
        ),
        ("table.input", json!({"kind": "strips", "text": "TBD"})),
        ("table.commit", json!({"kind": "strips"})),
        ("navigator.select", json!({"kind": "strips", "keys": bridged})),
        ("table.fill_down", json!({"kind": "strips"})),
        ("file.save", json!({})),
        ("file.close", json!({})),
    ]);
    all_done(&run(&exe, &tmp, "se6.jsonl", &script, None));

    let (s, _) = Session::open_read_only(&job, "test").expect("the saved project");
    let bom = parse_csv(
        &s.export_csv(Export::Bom(BomMode::Component))
            .expect("the parts list"),
    );
    let oracle = parse_csv(&std::fs::read_to_string(Path::new(ORACLE)).expect("the oracle"));
    for (description, qty) in [("End bracket", "4"), ("Plug-in bridge", "3")] {
        let expected = rows_of(&oracle, description);
        assert_eq!(expected.len(), 1, "{description} in the oracle");
        assert_eq!(expected[0][0], qty, "{description}: the oracle's quantity");
        assert_eq!(rows_of(&bom, description), expected, "{description}");
    }
    s.close();
}
