// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1636: the device and symbol browser's model: its tree of categories from the place catalogue's data, and a
//! part as one row that opens to its functions.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use ew_viewmodel::tools::{Node, PickerRow};
use serde_json::json;

/// F28: the tree is All, In this project, the trades, Other; a trade opens to its sub-groups, the categories. Each
/// node lists as many rows as its count says: a trade the parts of its sub-groups, a category node only parts of that
/// category, In this project only the project's own copies; every node has its words; `step` moves down the tree.
#[test]
fn the_browsers_tree_lists_each_trade_and_category_with_its_count() {
    let tmp = Tmp::new("browser-tree");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place_part", json!({}));
    let closed = s.picker().expect("the browser").nodes.clone();
    assert_eq!(closed[0].0, Node::All, "{closed:?}");
    assert!(
        closed.iter().all(|(n, _)| !matches!(n, Node::Category(_))),
        "the trades stand closed: {closed:?}"
    );
    let words: Vec<String> = closed
        .iter()
        .filter(|(n, _)| matches!(n, Node::Trade(_)))
        .map(|(n, _)| n.words())
        .collect();
    assert_eq!(
        words,
        ["Electrical", "PLC and bus", "Pneumatics", "Enclosure and mounting"]
    );
    // Every trade opened, the categories stand under their trade.
    for (node, _) in closed.iter().filter(|(n, _)| matches!(n, Node::Trade(_))) {
        done(&mut s, "picker.category", json!({ "to": node.word() }));
    }
    let nodes = s.picker().expect("the browser").nodes.clone();
    let mut trade = None;
    for (node, _) in &nodes {
        match node {
            Node::Trade(t) => trade = Some(*t),
            Node::Category(_) => assert_eq!(node.trade(), trade, "{node:?} under its trade: {nodes:?}"),
            _ => {}
        }
    }
    assert!(
        nodes.iter().any(|(n, _)| matches!(n, Node::Category(_))),
        "{nodes:?}"
    );
    for (node, count) in &nodes {
        done(&mut s, "picker.category", json!({ "to": node.word() }));
        let p = s.picker().expect("the browser");
        assert_eq!(p.node, *node);
        let parts: Vec<_> = p
            .rows
            .iter()
            .filter_map(|r| match r {
                PickerRow::Part(i) => Some(&p.catalogue.parts[*i]),
                _ => None,
            })
            .collect();
        assert_eq!(parts.len(), *count, "{node:?}");
        for part in parts {
            match node {
                Node::Category(key) => assert_eq!(&part.category, key, "{}", part.part_number),
                Node::Trade(t) => assert_eq!(
                    ew_viewmodel::tools::trade_of(&part.category),
                    Some(*t),
                    "{}",
                    part.part_number
                ),
                Node::Project => assert!(part.library.is_none(), "{}", part.part_number),
                _ => {}
            }
        }
        assert!(!node.words().is_empty(), "{node:?} in words");
    }
    done(&mut s, "picker.category", json!({"to": "all"}));
    done(&mut s, "picker.category", json!({"step": 1}));
    assert_eq!(s.picker().expect("the browser").node, nodes[1].0);
    s.shutdown();
}

/// F28: every function category the browser names in words has its trade, but the three that are no trade's
/// (assemblies and black boxes, interruption points, graphics), which stand under Other. A category added without a
/// trade would stand under Other unseen.
#[test]
fn every_named_category_has_its_trade() {
    let other = ["Assembly", "InterruptionPoint", "Graphic"];
    for (key, words) in ew_viewmodel::tools::function_categories() {
        assert_eq!(
            ew_viewmodel::tools::trade_of(key).is_none(),
            other.contains(&key),
            "{key} ({words})"
        );
    }
}

/// F28: a filter searches the whole tree and opens every trade with hits, so the tree shows where they sit.
#[test]
fn a_filter_shows_under_which_trade_and_group_the_hits_sit() {
    let tmp = Tmp::new("browser-filter");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.filter", json!({"text": "relay"}));
    let nodes = s.picker().expect("the browser").nodes.clone();
    let at = |n: &Node| nodes.iter().position(|(x, _)| x == n);
    let electrical = at(&Node::Trade(ew_viewmodel::tools::Trade::Electrical)).expect("Electrical");
    let relays = at(&Node::Category("Relay".to_owned())).expect("Relays shows under the filter");
    assert!(relays > electrical, "{nodes:?}");
    assert!(nodes[relays].1 > 0, "{nodes:?}");
    s.shutdown();
}

/// A part with several functions is one row; `picker.open` lists its functions under it, closing it from a function
/// row makes the part's row current again, and choosing the part's row arms the Place tool with its main function.
#[test]
fn a_part_is_one_row_that_opens_to_its_functions_and_places_its_main_function() {
    let tmp = Tmp::new("browser-part");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.filter", json!({"text": "EW-GEN-CON-1"}));
    let p = s.picker().expect("the browser");
    let [PickerRow::Part(part)] = p.rows[..] else {
        panic!("one row: {:?}", p.rows)
    };
    let functions = p.catalogue.parts[part].functions.len();
    assert!(functions > 1);
    assert!(
        p.row_line(p.rows[0]).ends_with(" closed"),
        "{}",
        p.row_line(p.rows[0])
    );
    done(&mut s, "picker.open", json!({}));
    let p = s.picker().expect("the browser");
    let opened: Vec<PickerRow> = std::iter::once(PickerRow::Part(part))
        .chain((0..functions).map(|k| PickerRow::PartFn(part, k)))
        .collect();
    assert_eq!(p.rows, opened);
    assert_eq!(p.row, 0, "the part's row stays current");
    done(&mut s, "picker.row", json!({"step": 2}));
    done(&mut s, "picker.open", json!({"open": false}));
    let p = s.picker().expect("the browser");
    assert_eq!((p.rows.len(), p.row), (1, 0), "closed, its part current");
    let main = p.part_main(part).expect("a main function");
    assert!(p.catalogue.parts[part].functions[main].main);
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    assert!(s.picker().is_none());
    assert_eq!(s.tool().name(), "place");
    s.shutdown();
}

/// FT-1530: in a Dutch window a starter symbol is named in Dutch.
#[test]
fn a_starter_symbol_is_named_in_the_windows_language() {
    let tmp = Tmp::new("browser-dutch");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    assert!(ew_viewmodel::strings::set_locale("nl"));
    done(&mut s, "tool.place", json!({}));
    let p = s.picker().expect("the browser");
    let name = p
        .catalogue
        .symbols
        .iter()
        .find(|x| x.key == "ew.iec.contact-no@qet10")
        .map(|x| x.name.clone());
    assert_eq!(name.as_deref(), Some("Maakcontact"));
    s.shutdown();
    assert!(ew_viewmodel::strings::set_locale("en"));
}

/// FT-1667: the starter contactor is under Contactors with the letter Q (IEC 81346-2), not under Relays with K.
#[test]
fn the_starter_contactor_is_under_contactors_with_the_letter_q() {
    let tmp = Tmp::new("browser-contactor");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.category", json!({"to": "SwitchingPower"}));
    let p = s.picker().expect("the browser");
    let contactor = p
        .rows
        .iter()
        .find_map(|r| match r {
            PickerRow::Part(i) if p.catalogue.parts[*i].part_number == "EW-GEN-CON-1" => {
                Some(&p.catalogue.parts[*i])
            }
            _ => None,
        })
        .expect("the contactor under Contactors");
    assert_eq!(contactor.letter.as_deref(), Some("Q"));
    s.shutdown();
}
