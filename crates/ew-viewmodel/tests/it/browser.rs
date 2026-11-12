// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1636: the device and symbol browser's model: its tree of categories from the place catalogue's data, and a
//! part as one row that opens to its functions.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use ew_viewmodel::tools::{Node, PickerRow};
use serde_json::json;

/// Each node of the tree lists as many rows as its count says: a category node only parts of that category, In this
/// project only the project's own copies; every node has its words; `step` moves down the tree.
#[test]
fn the_browsers_tree_lists_each_category_with_its_count() {
    let tmp = Tmp::new("browser-tree");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place_part", json!({}));
    let nodes = s.picker().expect("the browser").nodes.clone();
    assert_eq!(nodes[0].0, Node::All, "{nodes:?}");
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
