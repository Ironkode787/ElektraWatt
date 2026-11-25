// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A strip cut on one page and pasted on another takes the tag of its new page (doc 06 §3.6). The paste's ghost says
//! so before the click, its toast offers Keep beside Undo, which freezes the old tag in one step, and a block put down
//! by the frame is moved in to stand inside the drawing area.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A new project with the schematic pages 101 and 102 and two terminals of a new strip placed on 101, selected.
fn two_terminals_on_101(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut s, &tmp.0.join("new.ew"), &[]);
    for folio in ["101", "102"] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": "multi_line", "title": "Terminals"}),
        );
    }
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(
        &mut s,
        "tool.place_terminal",
        json!({"strip": "new", "count": "2"}),
    );
    let p = middle(&s);
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let ids: Vec<String> = tables(&s)
        .reps
        .iter()
        .filter(|r| r.terminal)
        .map(|r| r.id.to_string())
        .collect();
    done(&mut s, "select.set", json!({ "ids": ids }));
    s
}

/// A sheet point in the page's drawing area.
fn middle(s: &ShellModel) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
}

#[test]
fn a_moved_strip_says_its_tag_follows_the_page_and_keep_freezes_the_old_one() {
    let tmp = Tmp::new("moved-strip-tag");
    let mut s = two_terminals_on_101(&tmp);
    done(&mut s, "edit.cut", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "edit.paste_clip", json!({}));
    let p = middle(&s);
    done(&mut s, "canvas.pointer", at(p, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 100}));
    let labels: Vec<String> = s.tool_overlay().labels().map(str::to_owned).collect();
    assert!(
        labels
            .iter()
            .any(|l| l.contains(" becomes ") && l.contains("a tag follows the page its device is drawn on")),
        "the ghost says it before the click: {labels:?}"
    );
    common::click(&mut s, p, json!({}));
    let toast = s.toast().cloned().expect("the paste's toast");
    assert_eq!(toast.actions(), ["undo", "keep_tag"]);
    let keep = toast.label("keep_tag");
    let old = keep
        .strip_prefix("Keep ")
        .expect("Keep names the old tag")
        .to_owned();
    // It says first what it moved, in place of the Cut's toast.
    assert!(
        toast.text.starts_with("Moved 2 terminals of ")
            && toast.text.contains(&format!(" to page 102. {old} is now ")),
        "{}",
        toast.text
    );
    let drawn = |s: &ShellModel| {
        tables(s)
            .cps
            .iter()
            .any(|c| c.label.starts_with(&format!("{old}:")))
    };
    assert!(!drawn(&s), "the strip took the page's tag");
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "keep_tag"}),
    );
    assert!(drawn(&s), "Keep froze the old tag");
}

#[test]
fn a_block_put_down_by_the_frame_is_moved_inside_the_drawing_area() {
    let tmp = Tmp::new("paste-inside");
    let mut s = two_terminals_on_101(&tmp);
    done(&mut s, "edit.copy", json!({}));
    let before = tables(&s).reps.len();
    done(&mut s, "edit.paste_clip", json!({}));
    // The sheet's top-left corner, in the frame's border: the block lands inside, not refused.
    common::click(&mut s, (2_000_000, 2_000_000), json!({}));
    assert_eq!(tables(&s).reps.len(), before + 2, "both terminals pasted");
}
