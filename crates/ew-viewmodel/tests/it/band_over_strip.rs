// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A rubber band drawn left to right selects every symbol whose drawing lies inside it, whatever its texts do: a
//! persona's band over a row of terminals left the first out, because the strip's designation it prints stood outside
//! the band.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

#[test]
fn a_band_over_a_row_of_terminals_selects_them_all() {
    let tmp = Tmp::new("band-over-strip");
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
        "tool.place_terminal",
        json!({"strip": "new", "count": "4"}),
    );
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "select.set", json!({"ids": []}));
    // The band: just round the symbols' bodies and pins, 0.3 mm clear; the first terminal's designation and number
    // stand beside it, outside the band.
    let t = tables(&s);
    let terminals: Vec<_> = t.reps.iter().filter(|r| r.terminal).collect();
    assert_eq!(terminals.len(), 4);
    let pins = t.cps.iter().filter(|c| terminals.iter().any(|r| r.id == c.rep));
    let xs = terminals
        .iter()
        .flat_map(|r| [r.body[0], r.body[2]])
        .chain(pins.clone().map(|c| c.at.0));
    let ys = terminals
        .iter()
        .flat_map(|r| [r.body[1], r.body[3]])
        .chain(pins.map(|c| c.at.1));
    let (x0, x1) = (xs.clone().min().unwrap(), xs.max().unwrap());
    let (y0, y1) = (ys.clone().min().unwrap(), ys.max().unwrap());
    let sn = &s.canvas().unwrap().page().snapper;
    // Drawn from below the row, clear of every pin, up and to the right.
    let from = sn.page_to_sheet((x0 - 300_000, y1 + 2_000_000));
    let to = sn.page_to_sheet((x1 + 300_000, y0 - 300_000));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.release", at(to, json!({})));
    let mut selected: Vec<String> = s.selection().items.iter().map(|e| e.0.to_string()).collect();
    selected.sort();
    let mut all: Vec<String> = terminals.iter().map(|r| r.id.to_string()).collect();
    all.sort();
    assert_eq!(selected, all, "every terminal, the first too");
}
