// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The stand-in part and a part of one's own are found where a person looks (persona runs): Type not known yet… in
//! the command search under the words a person types and in the Parts row's menu beside Choose part…, opening the part
//! dialog with its field; Make variable says what it is; Remove from my parts library… in the search under "delete
//! part" and "remove part"; New part… on the PLC tab. On the sample's 102, its contactor -102Q1.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::command_bar::command_bar;
use ew_viewmodel::properties::{RowMenuItem, menu_tip};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

fn found(s: &mut ShellModel, query: &str) -> Vec<String> {
    if s.palette().is_none() {
        done(s, "tools.palette", json!({}));
    }
    s.set_palette_query(query);
    s.palette_entries().into_iter().map(|e| e.id).collect()
}

#[test]
fn the_stand_in_part_and_an_own_part_are_found_where_a_person_looks() {
    let tmp = Tmp::new("stand-in-own-parts");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    for q in ["placeholder", "stand-in", "not known", "undecided", "TBD"] {
        assert!(found(&mut s, q).iter().any(|id| id == "part.placeholder"), "{q}");
    }
    for q in ["delete part", "remove part"] {
        assert!(found(&mut s, q).iter().any(|id| id == "part.remove_own"), "{q}");
    }
    // A person who wants a PLC card of his own asks for it by those words too (persona re-test).
    for q in ["new part", "card", "module"] {
        assert!(found(&mut s, q).iter().any(|id| id == "part.new"), "{q}");
    }
    let _ = s.dispatch("ui.dismiss", &json!({}));

    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "select.set", json!({"refs": ["tag:-102Q1"]}));
    let menu = s.row_menu("device/parts");
    let choose = menu
        .iter()
        .position(|i| matches!(i, RowMenuItem::ChoosePart { .. }))
        .expect("Choose part…");
    assert_eq!(menu.get(choose + 1), Some(&RowMenuItem::Placeholder), "{menu:?}");
    assert!(menu_tip(RowMenuItem::MakeVariable).is_some_and(|t| t.contains("not a stand-in part")));

    // Type not known yet… opens the part dialog of the selection with its field open.
    done(&mut s, "part.placeholder", json!({}));
    assert!(
        matches!(s.dialog(), Some(Dialog::PartChoose(c)) if c.naming),
        "{:?}",
        s.dialog().map(|_| "another dialog")
    );
    let _ = s.dispatch("ui.dismiss", &json!({}));

    done(&mut s, "view.command_tab", json!({"tab": "plc"}));
    assert!(bar_offers(&s, "part.new"), "the PLC tab offers New part…");
}

/// Whether the bar on show offers `id`, as a button or in a split button's variants (New station…'s).
fn bar_offers(s: &ShellModel, id: &str) -> bool {
    command_bar(s).groups.iter().flat_map(|g| &g.buttons).any(|b| {
        b.id == id
            || matches!(&b.kind, ew_viewmodel::command_bar::ButtonKind::Split(v) if v.iter().any(|i| i.id == id))
    })
}
