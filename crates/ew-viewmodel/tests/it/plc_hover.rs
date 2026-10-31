// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 2 (D7) in the view model: `ui.describe` reports the printed PLC address of the connection point under
//! the pointer. On a writable copy of Golden A a spare channel of a valve terminal is bound to a drawn port of one of
//! its valves on folio 203; hovering that port prints `canvas hover cp "<label>" address "<address>"`, the address the
//! page tables give the CP. No such line is printed before the binding.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::output::TableCp;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

fn hover_line(s: &ShellModel) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with("canvas hover cp "))
        .map(str::to_owned)
}

/// The channel records of the project copy (`devices/*.jsonl`).
fn channels(tmp: &Tmp) -> Vec<Value> {
    let mut out = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(tmp.golden_copy().join("devices"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            if v["$k"] == "channel" {
                out.push(v);
            }
        }
    }
    out
}

fn table_cp(s: &ShellModel, id: &str) -> TableCp {
    s.canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .find(|c| c.id.to_string() == id)
        .expect("the CP on the page")
        .clone()
}

#[test]
fn describe_reports_the_plc_address_of_the_hovered_cp() {
    let tmp = Tmp::new("plc-hover");
    let mut s = tmp.shell_in(Mode::Headless, true, "203");
    done(&mut s, "view.zoom_actual", json!({}));
    // A drawn port of a valve whose coil a channel binds, and a spare channel of that channel's module.
    let records = channels(&tmp);
    let cps: Vec<TableCp> = s.canvas().unwrap().page().tables.cps.clone();
    let (target, channel) = cps
        .iter()
        .find_map(|c| {
            let function = c.function.to_string();
            let coil = records
                .iter()
                .find(|r| r["binding"]["cp"][0] == function.as_str())?;
            if coil["binding"]["cp"][1] == c.idx {
                return None;
            }
            let spare = records
                .iter()
                .filter(|r| r["module"] == coil["module"] && r.get("binding").is_none())
                .filter_map(|r| r["id"].as_str())
                .min()?;
            Some((c.clone(), spare.to_owned()))
        })
        .expect("a valve port and a spare channel on 203");
    let p = s.canvas().unwrap().page().snapper.page_to_sheet(target.at);
    done(&mut s, "canvas.pointer", at(p, json!({})));
    assert_eq!(hover_line(&s), None, "no address before the binding");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "plc.bind_channel", "params": {
            "channel": channel,
            "endpoint": {"cp": {"function": target.function.to_string(), "idx": target.idx}},
        }}),
    );
    let row = table_cp(&s, &target.id.to_string());
    let address = row
        .plc_address
        .clone()
        .expect("the bound CP has a printed address");
    assert!(address.starts_with('Q'), "{address}");
    done(&mut s, "canvas.pointer", at(p, json!({})));
    assert_eq!(
        hover_line(&s),
        Some(format!("canvas hover cp \"{}\" address \"{address}\"", row.label))
    );
    s.shutdown();
}
