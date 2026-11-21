// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1838: the station view's channel table has a keyboard cursor as the window's other tables have: the cell last
//! clicked or typed in keeps it, F2 opens the cell it stands on, the arrows walk the cells, and a place chosen in the
//! rack takes the keys back, so F2 changes a tag there again.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// Runs what the keyboard binding of `action` runs in the view (`shortcut_action`).
fn key(s: &mut ShellModel, action: &'static str) {
    let (name, params) = s.shortcut_action(action);
    done(s, name, params);
}

/// The channel cell open for typing: (channel, column, text).
fn open_cell(s: &ShellModel) -> Option<(String, &'static str, String)> {
    let (channel, column, text) = s.station_tab()?.channel.clone()?;
    Some((channel.to_string(), column, text))
}

/// The sample's controller in its view, its first card under the cursor: the card's first two channels.
#[test]
fn f2_opens_the_channel_cell_the_keyboard_cursor_stands_on() {
    let tmp = Tmp::new("station-channel-f2");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-104A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let channels: Vec<String> = s
        .station_shown()
        .expect("the view")
        .channels
        .iter()
        .map(|c| c.channel.to_string())
        .collect();
    let [first, second, ..] = channels.as_slice() else {
        panic!("a card of two channels: {channels:?}")
    };
    // A click on the first channel's Symbol cell, a name typed and Enter: the field closes.
    done(
        &mut s,
        "plc.rack_channel",
        json!({"channel": first, "column": "symbol"}),
    );
    done(&mut s, "plc.rack_channel", json!({"text": "B_START"}));
    assert_eq!(open_cell(&s), None, "the field closes on Enter");
    // F2 opens the cell again, on its text.
    key(&mut s, "text.edit");
    assert_eq!(
        open_cell(&s),
        Some((first.clone(), "symbol", "B_START".to_owned())),
        "F2 on the cell the cursor stands on"
    );
    done(&mut s, "plc.rack_channel", json!({"cancel": true}));
    // Down and Right: the next channel's Function text cell, which F2 opens or refuses in words.
    key(&mut s, "cursor.down");
    key(&mut s, "cursor.right");
    assert_eq!(
        s.station_channel_cursor().map(|(c, col)| (c.to_string(), col)),
        Some((second.clone(), "function_text"))
    );
    let (name, _) = s.shortcut_action("text.edit");
    assert_eq!(name, "plc.rack_channel", "F2 acts on the channel cell");
    // A place chosen in the rack takes the keys back: F2 changes the tag there.
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    assert_eq!(s.shortcut_action("text.edit").0, "plc.rack_rename");
    s.shutdown();
}
