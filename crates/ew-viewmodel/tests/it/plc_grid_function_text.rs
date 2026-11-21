// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1846: the PLC grid's Function text cells are the wired devices' function texts for every gesture: fill down
//! and paste set them as typing does, and a cell of a channel nothing is wired to is refused in words.

use crate::common::{Tmp, done};
use ew_viewmodel::ew_api::output::NavRole;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::navigators::table::CellValue;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The sample's PLC table in table mode, on a writable copy.
fn grid(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    s
}

/// The channel rows on show: (key, Target cell, Function text cell), in display order.
fn channels(s: &ShellModel) -> Vec<(String, String, String)> {
    let n = s.nav(NavKind::Plc).expect("the PLC navigator");
    let t = n.table().expect("its table");
    let col = |key: &str| t.columns.iter().position(|c| c.key == key).expect(key);
    let (target, text) = (col("target"), col("function_text"));
    n.visible()
        .iter()
        .map(|r| &t.nodes[r.node as usize])
        .filter(|node| node.role == NavRole::Channel)
        .map(|node| (node.key.clone(), node.cells[target].clone(), node.cells[text].clone()))
        .collect()
}

/// The Function text cell of the row `key`.
fn text_of(s: &ShellModel, key: &str) -> String {
    channels(s)
        .into_iter()
        .find(|c| c.0 == key)
        .expect("the row")
        .2
}

/// Typing into a wired channel's Function text cell, then a fill down from it over a second wired channel: both
/// wired devices print the typed text, each gesture one command on the devices' function texts.
#[test]
fn fill_down_sets_the_wired_devices_function_texts() {
    let tmp = Tmp::new("plc-grid-fill-down");
    let mut s = grid(&tmp);
    let wired: Vec<String> = channels(&s)
        .into_iter()
        .filter(|c| !c.1.is_empty())
        .map(|c| c.0)
        .collect();
    let [first, second, ..] = wired.as_slice() else {
        panic!("two wired channels: {wired:?}")
    };
    done(
        &mut s,
        "table.cell",
        json!({"kind": "plc", "row": first, "column": "function_text"}),
    );
    done(&mut s, "table.input", json!({"kind": "plc", "text": "Conveyor run"}));
    done(&mut s, "table.commit", json!({"kind": "plc"}));
    assert_eq!(text_of(&s, first), "Conveyor run", "typed");
    done(
        &mut s,
        "navigator.select",
        json!({"kind": "plc", "keys": [first, second]}),
    );
    let steps = s.recorded_commands().len();
    done(&mut s, "table.fill_down", json!({"kind": "plc"}));
    assert!(s.table_gesture().is_none(), "no card: {:?}", s.table_gesture());
    assert_eq!(s.recorded_commands().len(), steps + 1, "one command");
    let last = s.recorded_commands().last().expect("the fill").clone();
    assert_eq!(
        (last.params["kind"].as_str(), last.params["path"].as_str()),
        (Some("device"), Some("function_text")),
        "{:?}",
        last.params
    );
    assert_eq!(text_of(&s, second), "Conveyor run", "filled down");
    s.shutdown();
}

/// A text pasted into a wired channel's Function text cell becomes its device's; pasted into a channel nothing is
/// wired to, the card says why in the station view's words.
#[test]
fn paste_sets_a_wired_devices_function_text_and_refuses_a_spare_channel() {
    let tmp = Tmp::new("plc-grid-paste");
    let mut s = grid(&tmp);
    let rows = channels(&s);
    let wired = rows.iter().find(|c| !c.1.is_empty()).expect("a wired channel").0.clone();
    let spare = rows.iter().find(|c| c.1.is_empty()).expect("a spare channel").0.clone();
    done(
        &mut s,
        "table.cell",
        json!({"kind": "plc", "row": wired, "column": "function_text"}),
    );
    done(&mut s, "table.paste", json!({"kind": "plc", "text": "Clamp open\r\n"}));
    assert!(s.table_gesture().is_none(), "no card: {:?}", s.table_gesture());
    assert_eq!(text_of(&s, &wired), "Clamp open");
    done(
        &mut s,
        "table.cell",
        json!({"kind": "plc", "row": spare, "column": "function_text"}),
    );
    done(&mut s, "table.paste", json!({"kind": "plc", "text": "Clamp shut\r\n"}));
    let card = s.table_gesture().expect("the card of the refused cell");
    match &card.cells[0].value {
        CellValue::Invalid(why) => assert!(why.starts_with("Nothing is wired to this channel yet"), "{why}"),
        other => panic!("refused in words: {other:?}"),
    }
    s.shutdown();
}
