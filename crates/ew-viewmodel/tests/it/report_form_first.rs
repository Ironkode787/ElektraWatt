// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Report…'s form opens on the parts list, the Output tab's own list, never on the first kind of the alphabet; and the
//! words "cable list" find the cable overview in the command search, whose Report… then opens on it (a persona run
//! read "Bus cable list" chosen and did not know the cable list it wanted was "Cable overview and diagrams").

use crate::common;

use common::{Tmp, done, form};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn report_opens_on_the_parts_list_and_cable_list_finds_the_cable_overview() {
    let tmp = Tmp::new("report-form-first");
    let mut s = tmp.sample_in(Mode::Gui, true, "103");
    done(&mut s, "report_block.new", json!({}));
    let kind = &form(&s).fields[0];
    assert_eq!(kind.value, "parts_list");
    assert_eq!(kind.options[0].1, "Parts list", "first in the list");
    done(&mut s, "ui.dismiss", json!({}));

    done(&mut s, "tools.palette", json!({}));
    s.set_palette_query("cable list");
    let at = s
        .palette_entries()
        .iter()
        .position(|e| e.id == "report_block.new")
        .expect("Report… found by \"cable list\"");
    s.select_palette_entry(at);
    done_palette(&mut s);
    assert_eq!(form(&s).fields[0].value, "cable_overview_diagrams");
    done(&mut s, "ui.dismiss", json!({}));

    // Found by its own label it opens on the parts list as from the bar.
    done(&mut s, "tools.palette", json!({}));
    s.set_palette_query("report");
    let at = s
        .palette_entries()
        .iter()
        .position(|e| e.id == "report_block.new")
        .expect("Report… found by its label");
    s.select_palette_entry(at);
    done_palette(&mut s);
    assert_eq!(form(&s).fields[0].value, "parts_list");
}

fn done_palette(s: &mut ew_viewmodel::shell::ShellModel) {
    assert_eq!(s.run_palette_selection(), ew_viewmodel::shell::Outcome::Done);
}
