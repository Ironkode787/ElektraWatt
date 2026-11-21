// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A number per level of a terminal block (doc 08 §6.3): `LevelSpec.designation`, optional and additive, round-trips
//! and a level without one is designated by its terminal; `Terminal::level_copies` gives a one-level terminal's
//! connection points once more per listed level and leaves a part's own levels alone.

use ew_schema::Terminal;

fn terminal(levels: &str) -> Terminal {
    let text = format!(
        r#"{{"strip":"0190a000-0000-7000-8000-000000000001","seq":5,"designation":"5"{levels}}}"#
    );
    let t: Terminal = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_eq!(serde_json::to_string(&t).unwrap(), text, "written back as read");
    t
}

#[test]
fn a_level_with_a_number_of_its_own_prints_it_and_one_without_prints_the_terminal_s() {
    let t = terminal(r#","levels":[{"level":1},{"level":2,"designation":"6"}]"#);
    assert_eq!(t.level_designation(Some(1)).as_str(), "5");
    assert_eq!(t.level_designation(Some(2)).as_str(), "6");
    assert_eq!(t.level_designation(None).as_str(), "5");
    assert_eq!(t.level_designation(Some(3)).as_str(), "5");
}

#[test]
fn the_listed_levels_copy_the_connection_points_of_the_one_level_they_carry() {
    let block = terminal(r#","levels":[{"level":1},{"level":2,"designation":"6"}]"#);
    // `ew.fn.terminal`: CPs 0 and 1 on level 1.
    assert_eq!(block.level_copies(&[(0, Some(1)), (1, Some(1))]), [(2, 0, 2), (3, 1, 2)]);
    // A rail foot without a level stays once; the span counts it.
    assert_eq!(
        block.level_copies(&[(0, Some(1)), (1, Some(1)), (2, None)]),
        [(3, 0, 2), (4, 1, 2)]
    );
    // A part whose pattern has both levels gives them itself.
    assert_eq!(
        block.level_copies(&[(0, Some(1)), (1, Some(1)), (2, Some(2)), (3, Some(2))]),
        []
    );
    // A terminal that lists no levels, or CPs without a level, add none.
    assert_eq!(terminal("").level_copies(&[(0, Some(1)), (1, Some(1))]), []);
    assert_eq!(block.level_copies(&[(0, None), (1, None)]), []);
    // An index past u16::MAX is left out, never wrapped.
    assert_eq!(block.level_copies(&[(u16::MAX - 1, Some(1))]), []);
}
