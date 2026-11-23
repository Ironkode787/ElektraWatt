// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cut and a paste on another page are a move (doc 06 §3.6): the sample's strip -X1021, its terminals cut on folio
//! 102 and pasted on folio 101, is the same strip, and the paste's toast says that its tag followed the page.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn terminals_cut_and_pasted_on_another_page_keep_their_strip_and_the_toast_names_its_new_tag() {
    let tmp = Tmp::new("cut-paste-retag");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    let refs: Vec<String> = (1..=8).map(|n| format!("tag:-X1021:{n}")).collect();
    done(&mut shell, "select.set", json!({ "refs": refs }));
    done(&mut shell, "edit.cut", json!({}));
    // The cut takes the wires to the devices it did not take and says so; none is left drawn at nothing on 102.
    let cut = shell.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        cut.starts_with("Cut ")
            && cut.contains(" terminals. ")
            && cut.contains(" wires that ended at them are removed with them"),
        "{cut}"
    );
    done(&mut shell, "pages.open", json!({"folio": "101"}));
    done(&mut shell, "edit.paste_in_place", json!({}));
    let toast = shell.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.starts_with("Moved 8 terminals of -X101")
            && toast.contains(" to page 101. -X1021 is now -X101"),
        "{toast}"
    );
    assert!(
        toast.contains("a tag follows the page its device is drawn on"),
        "{toast}"
    );
    assert!(shell.toast().is_some_and(|t| t.action().is_some()), "with Undo");
    // The same strip: its terminals are drawn here, none is left unplaced on 102.
    let labels: Vec<String> = common::tables(&shell)
        .cps
        .iter()
        .map(|c| c.label.clone())
        .filter(|l| l.contains("-X101"))
        .collect();
    assert!(!labels.is_empty(), "the strip's terminals on 101");
    shell.shutdown();
}
