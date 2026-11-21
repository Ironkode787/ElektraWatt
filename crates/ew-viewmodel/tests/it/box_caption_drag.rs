// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dragging the part number of a drawn power supply's box, with
//! nothing selected, moved the whole box. The text under the pointer is dragged by itself; the box stays.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

#[test]
fn a_boxs_part_number_is_dragged_by_itself() {
    let tmp = Tmp::new("box-caption-drag");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-PSU-1"}));
    let sheet = |s: &ShellModel, k: (i64, i64)| {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    for k in [(40, 30), (52, 40)] {
        let p = sheet(&s, k);
        common::click(&mut s, p, json!({}));
    }
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "select.set", json!({"ids": []}));
    let psu = tables(&s).reps.last().cloned().expect("the box");
    let page = s.canvas().unwrap().page();
    let caption = page
        .texts
        .iter()
        .filter(|t| t.rep == psu.id)
        .min_by_key(|t| t.rect.min().y)
        .copied()
        .expect("the box's top caption");
    let mid = (
        (caption.rect.min().x.0 + caption.rect.max().x.0) / 2,
        (caption.rect.min().y.0 + caption.rect.max().y.0) / 2,
    );
    let from = page.snapper.page_to_sheet(mid);
    let to = page
        .snapper
        .page_to_sheet((mid.0 + 20_000_000, mid.1 - 10_000_000));
    let rev = s.revision();
    done(&mut s, "canvas.pointer", at(from, json!({})));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_ne!(s.revision(), rev, "one commit");
    let last = s.recorded_commands().last().cloned().expect("a command");
    assert_eq!(last.cmd, "rep.text_set", "{last:?}");
    let after = tables(&s);
    let box_after = after.reps.iter().find(|r| r.id == psu.id).expect("the box");
    assert_eq!(box_after.body, psu.body, "the box stays");
    s.shutdown();
}
