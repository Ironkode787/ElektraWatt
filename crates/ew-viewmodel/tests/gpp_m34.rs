// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP placement and interruption-point follow-ups of cycle 34, lane B34 (FT-399, FT-543, FT-545, FT-546,
//! FT-547, FT-550, FT-578), through the text interface.

mod common;

use common::{Tmp, done, state};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// A writable copy of Golden A with `folio` shown, headless.
fn golden(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

/// The first line of `ui.describe` that starts with `prefix`.
fn describe_line(s: &ShellModel, prefix: &str) -> String {
    let text = s.describe();
    text.lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}:\n{text}"))
        .to_owned()
}

/// The toast's text, empty without one.
fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

fn ghost(s: &ShellModel) -> Value {
    state(s)["canvas"]["ghost"].clone()
}

// ---------------------------------------------------------------------------------------------------------------
// FT-543: the picker's `terminal_designation`, `designation` its old name.

/// `terminal_designation` names the terminal's designation in `place.pick` (and `picker.choose`); `designation`, its
/// old name, still does; given both, `terminal_designation` wins.
#[test]
fn ft543_terminal_designation_names_the_terminal_and_the_old_name_still_works() {
    let tmp = Tmp::new("m34-543");
    let mut s = golden(&tmp, "103");
    let pick = |s: &mut ShellModel, params: Value| {
        done(s, "place.pick", params);
        let line = describe_line(s, "canvas ghost place");
        done(s, "tool.select", json!({}));
        line
    };
    let symbol = "ew.iec.terminal@qet10";
    let new = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "terminal_designation": "30"}),
    );
    assert!(new.contains("strip \"-X1031:30\""), "{new}");
    let old = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "designation": "31"}),
    );
    assert!(old.contains("strip \"-X1031:31\""), "{old}");
    let both = pick(
        &mut s,
        json!({"symbol": symbol, "strip": "-X1031", "terminal_designation": "32", "designation": "33"}),
    );
    assert!(both.contains("strip \"-X1031:32\""), "{both}");
}

/// A designation given to a device symbol in a project without strips (gpp-r2 sent `designation=S51` for a switch)
/// is refused as a terminal with no strip, and the sentence says where a device's tag is set.
#[test]
fn ft543_a_designation_on_a_device_says_where_its_tag_is_set() {
    let tmp = Tmp::new("m34-543b");
    let mut s = new_project(&tmp);
    for name in ["terminal_designation", "designation"] {
        let out = s.dispatch(
            "place.pick",
            &json!({"symbol": "ew.iec.position-switch-no@qet10", name: "S51"}),
        );
        assert_eq!(
            out,
            Outcome::Refused {
                reason_key: "reason-no-strip"
            },
            "{name}"
        );
    }
    let text = strings::text("reason-no-strip");
    assert!(text.contains("terminal designation"), "{text}");
    assert!(text.contains("a device's tag is set in its properties"), "{text}");
}
