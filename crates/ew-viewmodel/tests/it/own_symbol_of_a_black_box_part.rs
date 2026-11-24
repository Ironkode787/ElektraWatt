// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part whose function is a black box in the starter set (the power supply `ew.fn.psu-ac-dc`) is placed by the
//! Place tool with a symbol of its own drawn for that function, instead of being refused there as a black box and by
//! the Black box tool as a symbol (FT-1865).

use crate::common::{Tmp, done, tables};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_part_on_the_power_supply_function_is_placed_with_its_own_symbol() {
    let tmp = Tmp::new("own-psu-symbol");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut shell, &tmp.0.join("project"), &[]);
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2, "grid": {"num": 4000000, "den": 1}}}),
    );
    done(&mut shell, "page.goto", json!({"folio": "2"}));
    done(
        &mut shell,
        "symbol.new",
        json!({"key": "own.psu", "name": "Own supply"}),
    );
    done(&mut shell, "symbol.function", json!({"key": "ew.fn.psu-ac-dc"}));
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -4, "y": 4, "x2": 20, "y2": 28}),
    );
    // The function's seven connection points: L, N and PE above, two + and two - below.
    let cps = [
        (0, 0, "L", "up"),
        (8, 0, "N", "up"),
        (16, 0, "PE", "up"),
        (0, 32, "+", "down"),
        (4, 32, "+", "down"),
        (12, 32, "-", "down"),
        (16, 32, "-", "down"),
    ];
    for (fn_cp, (x, y, designation, direction)) in cps.into_iter().enumerate() {
        done(
            &mut shell,
            "symbol.cp",
            json!({"x": x, "y": y, "designation": designation, "direction": direction, "fn_cp": fn_cp}),
        );
    }
    done(
        &mut shell,
        "symbol.slot",
        json!({"kind": "tag", "x": -30, "y": 10}),
    );
    done(
        &mut shell,
        "symbol.save",
        json!({"directory": tmp.0.join("symbols.ewlib")}),
    );
    done(&mut shell, "symbol.close", json!({}));

    let before = tables(&shell).reps.len();
    done(&mut shell, "tool.place_part", json!({}));
    done(&mut shell, "place.pick", json!({"part": "EW-GEN-PSU-1"}));
    done(&mut shell, "cursor.set", json!({"x_mm": 120, "y_mm": 80}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(
        tables(&shell).reps.len(),
        before + 1,
        "the supply is drawn with its own symbol"
    );
}
