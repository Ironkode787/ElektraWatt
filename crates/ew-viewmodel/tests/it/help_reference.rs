// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F1 for what is under the hand (rc.3) on a copy of Golden A: a wire under the pointer opens the wires' section, the
//! selected wire under it its route's, a terminal the terminals'; the status bar says what F1 opens; a row of Properties
//! opens its anchor of the properties reference, whose page `cargo xtask docs-gen` writes from the reference this
//! file's last test writes.

use std::path::PathBuf;

use crate::common::{Tmp, at, done};
use ew_viewmodel::help::{self, HelpUnder};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::status::{self, StatusKind};
use serde_json::json;

/// The page F1 opens now, as the window asks for it: what is under the pointer, else the context.
fn f1(shell: &mut ShellModel) -> String {
    let params = help::pointer_topic(shell).map_or(json!({}), |u| json!({ "topic": u.topic }));
    done(shell, "help.documentation", params);
    shell.last_help().unwrap().to_owned()
}

/// The pointer at the page point `p` of the page on show.
fn point_at(shell: &mut ShellModel, p: (i64, i64)) {
    let q = shell.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(shell, "canvas.pointer", at(q, json!({})));
}

/// The middle of the first segment of the page's first wire, and the wire's ID.
fn a_wire(shell: &ShellModel) -> ((i64, i64), String) {
    let page = shell.canvas().unwrap().page();
    let wire = page
        .tables
        .connections
        .iter()
        .find(|c| c.kind == "wire" && c.line_kind.is_none() && c.polyline.len() >= 2)
        .expect("a wire on the page");
    let (a, b) = (wire.polyline[0], wire.polyline[1]);
    ((a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2), wire.id.to_string())
}

#[test]
fn f1_opens_the_section_of_the_thing_under_the_pointer() {
    let tmp = Tmp::new("f1-things");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let (middle, wire) = a_wire(&shell);
    point_at(&mut shell, middle);
    let under = help::pointer_topic(&shell).expect("the wire under the pointer");
    assert_eq!(under.subject, "Wires");
    // The status bar says what F1 opens there.
    let fields = status::bar(&shell, false);
    let help_field = fields.iter().find(|f| f.kind == StatusKind::Help).expect("the F1 field");
    assert_eq!(help_field.text, "F1: Wires");
    assert_eq!(f1(&mut shell), "reference/window/things.html#wires");
    // The selected wire under the pointer: its handles change its route.
    done(&mut shell, "select.set", json!({ "ids": [wire] }));
    point_at(&mut shell, middle);
    assert_eq!(
        f1(&mut shell),
        "reference/window/tools.html#change-a-wires-route-by-hand"
    );
    // A terminal by what it is, on the first page that draws one.
    done(&mut shell, "select.set", json!({ "ids": [] }));
    let folios: Vec<String> = shell
        .page_rows()
        .iter()
        .filter(|r| !r.generated)
        .map(|r| r.folio.clone())
        .collect();
    let terminal = folios
        .iter()
        .find_map(|folio| {
            done(&mut shell, "page.goto", json!({ "folio": folio }));
            let page = shell.canvas()?.page();
            let rep = page.tables.reps.iter().find(|r| r.terminal)?;
            // Its body's middle: a terminal's insertion point is one of its connection points.
            let b = rep.body;
            Some(((b[0] + b[2]) / 2, (b[1] + b[3]) / 2))
        })
        .expect("a page that draws a terminal");
    point_at(&mut shell, terminal);
    assert_eq!(
        f1(&mut shell),
        "reference/window/things.html#terminals-and-terminal-strips"
    );
    // What the window names under the pointer wins over the page; a dialog's own topic over the page behind it.
    shell.set_help_under(Some(HelpUnder {
        topic: "route-reroute".to_owned(),
        subject: "Reroute".to_owned(),
    }));
    assert_eq!(f1(&mut shell), "reference/window/tools.html#reroute");
    shell.set_help_under(None);
    done(&mut shell, "nav.goto", json!({}));
    assert_eq!(help::pointer_topic(&shell), None);
    assert_eq!(f1(&mut shell), "reference/window/dialogs.html#go-to-page-or-cell");
    shell.shutdown();
}

/// The pointer on the frame's title block, which holds no thing of the drawing, opens the frame's section.
#[test]
fn f1_on_the_frame_opens_the_frames_section() {
    let tmp = Tmp::new("f1-frame");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let (w, h) = shell.canvas().unwrap().page().sheet;
    // The title block's band at the sheet's foot, below the frame's cells.
    let title_block = (w / 2, h - 10_000_000);
    let cells = shell.canvas().unwrap().page().cells.clone().expect("a frame");
    assert!(cells.cell_at(title_block).is_none(), "the title block is outside the cells");
    done(&mut shell, "canvas.pointer", at(title_block, json!({})));
    assert_eq!(f1(&mut shell), "reference/window/things.html#the-frame-and-title-block");
    shell.shutdown();
}

/// With the pointer on nothing, F1 answers for the selection, then the tool: a selected wire opens the wires' section.
#[test]
fn f1_without_a_pointer_opens_the_selection_then_the_tool() {
    let tmp = Tmp::new("f1-selection");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    let (_, wire) = a_wire(&shell);
    done(&mut shell, "select.set", json!({ "ids": [wire] }));
    done(&mut shell, "ui.focus", json!({"area": "canvas"}));
    assert_eq!(f1(&mut shell), "reference/window/things.html#wires");
    done(&mut shell, "select.set", json!({ "ids": [] }));
    done(&mut shell, "tool.wire", json!({}));
    assert_eq!(f1(&mut shell), "reference/window/tools.html#wire");
    shell.shutdown();
}

/// F1 on the cursor row of Properties opens the row's place in the properties reference.
#[test]
fn f1_on_a_row_of_properties_opens_its_anchor() {
    let tmp = Tmp::new("f1-props");
    let mut shell = tmp.shell_in(Mode::Gui, true, "102");
    done(&mut shell, "select.set", json!({"refs": ["tag:-102G1"]}));
    done(&mut shell, "props.show", json!({"on": true}));
    done(&mut shell, "ui.focus", json!({"area": "properties"}));
    done(&mut shell, "props.row", json!({"to": "device/process_tag"}));
    assert_eq!(
        f1(&mut shell),
        "reference/properties.html#device-process-tag"
    );
    shell.shutdown();
}

/// The properties reference (NFR-DOC-06): written where `EW_REFERENCE_DIR` asks for the reference, for `cargo xtask
/// docs-gen`'s `reference/properties.md`.
// The variable only chooses whether the generated file is also written out.
#[allow(clippy::disallowed_methods)]
#[test]
fn the_properties_reference() {
    let sections: Vec<serde_json::Value> = help::property_sections()
        .into_iter()
        .map(|s| {
            json!({
                "kind": s.kind,
                "anchor": s.anchor,
                "title": s.title,
                "rows": s.rows.into_iter().map(|r| json!({
                    "anchor": r.anchor,
                    "label": r.label,
                    "help": r.help,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&json!({ "sections": sections })).unwrap();
    if let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("properties.json"), &text).unwrap();
    }
    let device = sections
        .iter()
        .find(|s| s["kind"] == "device")
        .expect("the device's section");
    assert_eq!(device["title"], "Device");
    assert!(
        device["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["anchor"] == "device-process-tag" && r["label"] == "Process tag"),
        "{device}"
    );
}
