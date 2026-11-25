// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire's Potential row opens Define potential… itself: Enter, F2 or a click on the row of a potential
//! no definition names is `props.edit`, which opens the same form as the row's "…" menu and the wire's menu.

use crate::common::{Tmp, done};
use crate::potential_define::undefined;
use ew_viewmodel::properties::RowMenuItem;
use serde_json::json;

#[test]
fn the_potential_row_of_an_undefined_potential_opens_define_potential() {
    let tmp = Tmp::new("potential-row-define");
    let mut shell = undefined(&tmp);
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let wire = shell.canvas().unwrap().page().tables.connections[0]
        .id
        .to_string();
    done(&mut shell, "select.set", json!({"ids": [wire]}));
    done(&mut shell, "props.show", json!({"on": true}));
    let key = shell
        .properties()
        .sheet()
        .and_then(|sheet| {
            sheet
                .sections
                .iter()
                .flat_map(|s| &s.rows)
                .find(|r| r.key.ends_with("/derived.potential"))
        })
        .map(|r| r.key.clone())
        .expect("the wire's Potential row");
    assert!(
        shell.row_menu(&key).contains(&RowMenuItem::DefinePotential),
        "{key}"
    );
    done(&mut shell, "props.edit", json!({"row": key}));
    let form = shell
        .palette()
        .and_then(|p| p.form.clone())
        .expect("Define potential…'s form");
    assert_eq!(form.action, "potential.draw");
}
