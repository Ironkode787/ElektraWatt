// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `place.pick` names an installed library's symbol by its key before its first placement as after it, and by its
//! address after it as before (FT-1865, FT-1860): a script need not know whether the project holds the symbol yet.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_library_symbol_is_named_by_its_key_and_its_address_before_and_after_its_first_placement() {
    let tmp = Tmp::new("symbol-by-key");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "file.new",
        json!({"dir": tmp.0.join("project"), "template": "builtin"}),
    );
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2, "grid": {"num": 4000000, "den": 1}}}),
    );
    done(&mut shell, "page.goto", json!({"folio": "2"}));
    done(
        &mut shell,
        "symbol.new",
        json!({"key": "example.by-key", "name": "Keyed contact"}),
    );
    done(
        &mut shell,
        "symbol.draw",
        json!({"tool": "rectangle", "x": 0, "y": 0, "x2": 8, "y2": 8}),
    );
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 0, "y": 0, "designation": "1"}),
    );
    done(
        &mut shell,
        "symbol.save",
        json!({"directory": tmp.0.join("symbols.ewlib")}),
    );
    done(&mut shell, "symbol.close", json!({}));
    done(&mut shell, "tool.place", json!({}));
    let address = shell
        .picker()
        .unwrap()
        .catalogue
        .symbols
        .iter()
        .find(|r| r.name.starts_with("Keyed contact"))
        .expect("the published symbol is offered")
        .key
        .clone();
    assert!(address.starts_with("library:"), "{address}");
    done(&mut shell, "ui.dismiss", json!({}));

    // Before its first placement: by its key.
    done(
        &mut shell,
        "place.pick",
        json!({"symbol": "example.by-key", "letter": "U"}),
    );
    done(&mut shell, "cursor.set", json!({"x_mm": 40, "y_mm": 40}));
    done(&mut shell, "canvas.activate", json!({}));
    done(&mut shell, "ui.dismiss", json!({}));

    // After it: by its key and by its address.
    for symbol in ["example.by-key", address.as_str()] {
        done(&mut shell, "place.pick", json!({"symbol": symbol, "letter": "U"}));
        done(&mut shell, "ui.dismiss", json!({}));
    }
}
