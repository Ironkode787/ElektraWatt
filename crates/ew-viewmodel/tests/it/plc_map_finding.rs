// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-PLC-012, a binding or an address that differs from the part's map, offers the fix its rule's catalogue names
//! beside the command fix (FT-1766): the module's part opened in the part editor, whose PLC section holds the map.

use crate::common::{Tmp, actual_size, done};
use ew_viewmodel::ew_api::output::NavRole;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Dialog, ShellModel};
use serde_json::json;

/// The key and the address of the first row of the PLC grid sorted by address.
fn first_channel(s: &ShellModel) -> (String, String) {
    let n = s.nav(NavKind::Plc).expect("the PLC navigator");
    let t = n.table().expect("its table");
    let column = t
        .columns
        .iter()
        .position(|c| c.key == "address")
        .expect("the address column");
    let node = &t.nodes[n.visible()[0].node as usize];
    (node.key.clone(), node.cells[column].clone())
}

/// A channel of the sample's controller given the address of its neighbour by hand deviates from its card's map:
/// EW-PLC-012 reports it, and its row offers Open the part's PLC section…, which opens the card's part in the part
/// editor.
#[test]
fn a_finding_against_the_part_s_map_opens_the_module_s_part() {
    let tmp = Tmp::new("plc-012-offer");
    let mut s = actual_size(&tmp, "104");
    done(&mut s, "navigator.show", json!({"kind": "plc"}));
    done(&mut s, "navigator.table", json!({"kind": "plc", "on": true}));
    done(&mut s, "navigator.sort", json!({"kind": "plc", "by": "address"}));
    let (row, address) = first_channel(&s);
    let (byte, bit) = address.rsplit_once('.').expect("a bit address");
    let neighbour = format!("{byte}.{}", bit.parse::<u8>().expect("a bit") + 1);
    // The first two channels swap their addresses in one paste, so no two share one.
    done(
        &mut s,
        "table.cell",
        json!({"kind": "plc", "row": row, "column": "address"}),
    );
    done(
        &mut s,
        "table.paste",
        json!({"kind": "plc", "text": format!("{neighbour}\r\n{address}\r\n")}),
    );
    done(&mut s, "messages.show", json!({"on": true}));
    let finding = s
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Finding && n.cells.get(1).is_some_and(|c| c == "EW-PLC-012"))
                .cloned()
        })
        .expect("EW-PLC-012 reports the deviation");
    let offer = s.finding_offer(&finding.key).expect("the window's fix");
    assert_eq!(offer.label, "Open the part's PLC section…");
    assert!(offer.enabled, "{:?}", offer.reason);
    done(&mut s, offer.id, offer.params.clone());
    match s.dialog() {
        Some(Dialog::PartEdit(e)) => assert_eq!(e.draft.plc.role, "module", "the card's part"),
        other => panic!("the part editor: {other:?}"),
    }
}
