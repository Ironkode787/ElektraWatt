// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Tube tool works as the Wire tool does (a click on a port ends the tube, as one on a pin ends a wire), and its
//! hint says so in a tube's words (a persona read "connection point" and "wire" on a fluid page and could not tell
//! which of his clicks ended a tube). The sample's fluid page 201 and its wiring page 101.

use crate::common;

use common::{Tmp, done, state};
use ew_viewmodel::shell::Mode;
use serde_json::json;

fn hint(s: &ew_viewmodel::shell::ShellModel) -> String {
    state(s)["status"][0].as_str().unwrap_or_default().to_owned()
}

#[test]
fn the_tube_tools_hint_speaks_of_ports_and_tubes() {
    let tmp = Tmp::new("tube-tool-words");
    let mut s = tmp.sample_in(Mode::Headless, true, "201");
    done(&mut s, "tool.wire", json!({}));
    let h = hint(&s);
    assert!(h.contains("on a port, a junction or a tube to start"), "{h}");
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "tool.wire", json!({}));
    let h = hint(&s);
    assert!(
        h.contains("on a connection point, a junction or a wire to start"),
        "{h}"
    );
}
