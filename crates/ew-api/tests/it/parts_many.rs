// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many parts at once (doc 07 §5.9): the fields several of his own parts share, "mixed" where they differ, one value
//! saved into all of them.

use crate::common;

use ew_api::{PartSheet, Session, SessionOptions};

/// A session over the libraries alone whose person has a user folder under `root`.
fn alone(root: &std::path::Path) -> Session {
    Session::libraries_only(SessionOptions {
        libraries: Some(common::tiers(root)),
        ..SessionOptions::default()
    })
}

/// A terminal of his own of `number`, `width` and `height` millimetres, saved into My parts; its sheet from there.
fn terminal(s: &mut Session, number: &str, width: &str, height: &str) -> PartSheet {
    let mut sheet = s.new_part_draft().expect("a new part");
    for (f, v) in [
        ("manufacturer", "Example"),
        ("part_number", number),
        ("kind", "terminal"),
        ("mech.width", width),
        ("mech.height", height),
    ] {
        s.set_draft_field(&mut sheet, f, v).expect(f);
    }
    let (lib, _) = s.new_part_in_my_parts(&sheet).expect("saved into My parts");
    s.library_part_draft(lib, sheet.copy).expect("its sheet")
}

#[test]
fn a_value_typed_into_several_parts_is_saved_into_each_and_mixed_is_left_as_each_has_it() {
    let tmp = ew_testdir::Tmp::new("parts-many");
    let mut s = alone(&tmp.0);
    let mut a = terminal(&mut s, "FT-4", "6.2", "60");
    let mut b = terminal(&mut s, "FT-6", "8.2", "70");
    let shared = ew_api::parts_many::shared_fields(&[a.clone(), b.clone()]);
    let field = |f: &str| shared.iter().find(|(x, _)| x == f).map(|(_, t)| t.clone());
    // Both are terminals: their sizes are shared fields, each different, so "mixed".
    assert_eq!(field("mech.width"), Some(None));
    assert_eq!(field("mech.height"), Some(None));
    assert_eq!(field("description"), Some(Some(String::new())));
    // What tells one from another is no shared field.
    assert_eq!(field("part_number"), None);
    for sheet in [&mut a, &mut b] {
        s.set_draft_field(sheet, "mech.depth", "45").expect("the depth");
    }
    assert!(
        s.own_parts_update(&[&a, &b]).expect("saved").is_none(),
        "no project holds them"
    );
    let lib = a.source.expect("from My parts").0;
    for (sheet, height) in [(&a, 60), (&b, 70)] {
        let now = s.library_part_draft(lib, sheet.source.unwrap().1).expect("saved");
        assert_eq!(now.text("mech.depth"), "45");
        assert_eq!(
            now.text("mech.height"),
            height.to_string(),
            "a mixed field stays as each has it"
        );
        assert_ne!(now.version, sheet.version, "its next version");
    }
}
