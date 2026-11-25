// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Copy pages… of a page with a bus device says what its copy still needs:
//! the card names the bus
//! devices, and the toast after Apply says to draw their bus line and give each its address. The sample's 201, its
//! valve terminal on PROFIBUS.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_copied_bus_device_is_said_to_need_its_bus_line_and_address() {
    let tmp = Tmp::new("copy-pages-bus");
    let mut s = tmp.sample_in(Mode::Headless, true, "201");
    done(
        &mut s,
        "pages.copy",
        json!({"pages": ["201"], "offset": 10, "rename": {"-201": "-211"}}),
    );
    let notices = s.preview_pane().notices().join("\n");
    assert!(
        notices.contains("Bus devices whose copies need a bus line and an address of their own: -201K1"),
        "{notices}"
    );
    done(&mut s, "preview.apply", json!({}));
    let said = toast(&s);
    assert!(
        said.starts_with("Copied 1 page: 211. The copies of -201K1 join no bus yet"),
        "{said}"
    );
}
